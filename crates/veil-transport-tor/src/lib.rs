//! Tor SOCKS5 fallback transport adapter for VEIL.
//!
//! This adapter is intentionally simple and best-effort:
//! - outbound sends only (`recv()` always returns `None`)
//! - each send opens a SOCKS5 stream, writes bytes, then closes
//! - intended as a censorship-resistant fallback lane

use std::str::FromStr;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use thiserror::Error;
use tokio::io::AsyncWriteExt;
use tokio::sync::{mpsc as tokio_mpsc, oneshot};
use tokio_socks::tcp::Socks5Stream;
use veil_transport::adapter::{TransportAdapter, TransportHealthSnapshot};

#[derive(Debug, Clone)]
pub struct TorSocksAdapterConfig {
    pub socks_proxy_addr: String,
    pub connect_timeout: Duration,
    pub send_timeout: Duration,
    pub outbound_queue_capacity: usize,
    pub max_payload_hint: Option<usize>,
}

impl TorSocksAdapterConfig {
    pub fn new(socks_proxy_addr: impl Into<String>) -> Self {
        Self {
            socks_proxy_addr: socks_proxy_addr.into(),
            connect_timeout: Duration::from_secs(8),
            send_timeout: Duration::from_secs(8),
            outbound_queue_capacity: 1024,
            max_payload_hint: None,
        }
    }
}

#[derive(Debug, Error)]
pub enum TorSocksAdapterError {
    #[error("adapter is closed")]
    Closed,
    #[error("outbound queue is full")]
    QueueFull,
    #[error("invalid peer format, expected host:port")]
    InvalidPeer,
    #[error("payload exceeds max payload hint ({hint} bytes)")]
    PayloadTooLarge { hint: usize },
}

#[derive(Debug)]
struct OutboundMessage {
    peer: String,
    bytes: Vec<u8>,
}

pub struct TorSocksAdapter {
    max_payload_hint: Option<usize>,
    outbound_tx: tokio_mpsc::Sender<OutboundMessage>,
    shutdown_tx: Option<oneshot::Sender<()>>,
    worker: Option<JoinHandle<()>>,
    running: Arc<AtomicBool>,
    metrics: Arc<TorSocksAdapterMetricsInner>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TorSocksAdapterMetrics {
    pub outbound_queued: u64,
    pub send_attempts: u64,
    pub send_success: u64,
    pub send_errors: u64,
}

#[derive(Debug, Default)]
struct TorSocksAdapterMetricsInner {
    outbound_queued: AtomicU64,
    send_attempts: AtomicU64,
    send_success: AtomicU64,
    send_errors: AtomicU64,
}

impl TorSocksAdapter {
    pub fn connect(config: TorSocksAdapterConfig) -> Result<Self, TorSocksAdapterError> {
        let (outbound_tx, outbound_rx) =
            tokio_mpsc::channel::<OutboundMessage>(config.outbound_queue_capacity);
        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
        let running = Arc::new(AtomicBool::new(true));
        let metrics = Arc::new(TorSocksAdapterMetricsInner::default());
        let worker_running = Arc::clone(&running);
        let worker_metrics = Arc::clone(&metrics);
        let worker_config = config.clone();

        let worker = thread::spawn(move || {
            let runtime = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(rt) => rt,
                Err(_) => {
                    worker_running.store(false, Ordering::Relaxed);
                    return;
                }
            };

            runtime.block_on(run_tor_worker(
                worker_config,
                worker_running,
                worker_metrics,
                outbound_rx,
                shutdown_rx,
            ));
        });

        Ok(Self {
            max_payload_hint: config.max_payload_hint,
            outbound_tx,
            shutdown_tx: Some(shutdown_tx),
            worker: Some(worker),
            running,
            metrics,
        })
    }

    pub fn metrics_snapshot(&self) -> TorSocksAdapterMetrics {
        TorSocksAdapterMetrics {
            outbound_queued: self.metrics.outbound_queued.load(Ordering::Relaxed),
            send_attempts: self.metrics.send_attempts.load(Ordering::Relaxed),
            send_success: self.metrics.send_success.load(Ordering::Relaxed),
            send_errors: self.metrics.send_errors.load(Ordering::Relaxed),
        }
    }
}

impl Drop for TorSocksAdapter {
    fn drop(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

impl TransportAdapter for TorSocksAdapter {
    type Peer = String;
    type Error = TorSocksAdapterError;

    fn send(&mut self, peer: &Self::Peer, bytes: &[u8]) -> Result<(), Self::Error> {
        parse_peer(peer)?;
        if let Some(hint) = self.max_payload_hint {
            if bytes.len() > hint {
                return Err(TorSocksAdapterError::PayloadTooLarge { hint });
            }
        }

        self.outbound_tx
            .try_send(OutboundMessage {
                peer: peer.clone(),
                bytes: bytes.to_vec(),
            })
            .map_err(|err| match err {
                tokio_mpsc::error::TrySendError::Full(_) => TorSocksAdapterError::QueueFull,
                tokio_mpsc::error::TrySendError::Closed(_) => TorSocksAdapterError::Closed,
            })
            .map(|_| {
                self.metrics.outbound_queued.fetch_add(1, Ordering::Relaxed);
            })
    }

    fn recv(&mut self) -> Option<(Self::Peer, Vec<u8>)> {
        None
    }

    fn max_payload_hint(&self) -> Option<usize> {
        self.max_payload_hint
    }

    fn can_send(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }

    fn can_recv(&self) -> bool {
        false
    }

    fn health_snapshot(&self) -> TransportHealthSnapshot {
        let m = self.metrics_snapshot();
        TransportHealthSnapshot {
            outbound_queued: m.outbound_queued,
            outbound_send_ok: m.send_success,
            outbound_send_err: m.send_errors,
            inbound_received: 0,
            inbound_dropped: 0,
            reconnect_attempts: 0,
            last_error: None,
            last_error_code: None,
        }
    }
}

fn parse_peer(peer: &str) -> Result<(String, u16), TorSocksAdapterError> {
    let idx = peer.rfind(':').ok_or(TorSocksAdapterError::InvalidPeer)?;
    let host = peer[..idx].trim();
    let port = peer[idx + 1..].trim();
    if host.is_empty() {
        return Err(TorSocksAdapterError::InvalidPeer);
    }
    let port = u16::from_str(port).map_err(|_| TorSocksAdapterError::InvalidPeer)?;
    if port == 0 {
        return Err(TorSocksAdapterError::InvalidPeer);
    }
    Ok((host.to_string(), port))
}

async fn run_tor_worker(
    config: TorSocksAdapterConfig,
    running: Arc<AtomicBool>,
    metrics: Arc<TorSocksAdapterMetricsInner>,
    mut outbound_rx: tokio_mpsc::Receiver<OutboundMessage>,
    mut shutdown_rx: oneshot::Receiver<()>,
) {
    loop {
        tokio::select! {
            _ = &mut shutdown_rx => {
                break;
            }
            maybe_msg = outbound_rx.recv() => {
                match maybe_msg {
                    Some(msg) => {
                        metrics.send_attempts.fetch_add(1, Ordering::Relaxed);
                        if let Ok((host, port)) = parse_peer(&msg.peer) {
                            let connect = tokio::time::timeout(
                                config.connect_timeout,
                                Socks5Stream::connect(config.socks_proxy_addr.as_str(), (host.as_str(), port)),
                            ).await;
                            if let Ok(Ok(mut stream)) = connect {
                                let write_res = tokio::time::timeout(config.send_timeout, stream.write_all(&msg.bytes)).await;
                                if matches!(write_res, Ok(Ok(()))) {
                                    metrics.send_success.fetch_add(1, Ordering::Relaxed);
                                } else {
                                    metrics.send_errors.fetch_add(1, Ordering::Relaxed);
                                }
                                if stream.shutdown().await.is_err() {
                                    metrics.send_errors.fetch_add(1, Ordering::Relaxed);
                                }
                            } else {
                                metrics.send_errors.fetch_add(1, Ordering::Relaxed);
                            }
                        } else {
                            metrics.send_errors.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                    None => break,
                }
            }
        }
    }

    running.store(false, Ordering::Relaxed);
}

#[cfg(test)]
mod tests {
    use super::{TorSocksAdapter, TorSocksAdapterConfig};
    use std::time::Duration;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;
    use veil_transport::adapter::TransportAdapter;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn tor_socks_adapter_sends_payload_via_proxy() {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("proxy bind should work");
        let proxy_addr = listener.local_addr().expect("proxy addr should exist");

        let (payload_tx, payload_rx) = tokio::sync::oneshot::channel::<Vec<u8>>();
        let server = tokio::spawn(async move {
            let (mut sock, _) = listener.accept().await.expect("proxy accept should work");

            // SOCKS5 method negotiation
            let mut hello = [0_u8; 3];
            sock.read_exact(&mut hello)
                .await
                .expect("hello should read");
            assert_eq!(hello[0], 0x05);
            sock.write_all(&[0x05, 0x00])
                .await
                .expect("method response should write");

            // CONNECT request: VER CMD RSV ATYP + DST + PORT
            let mut head = [0_u8; 4];
            sock.read_exact(&mut head)
                .await
                .expect("connect head should read");
            assert_eq!(head[0], 0x05);
            assert_eq!(head[1], 0x01);
            let atyp = head[3];
            match atyp {
                0x03 => {
                    let mut len = [0_u8; 1];
                    sock.read_exact(&mut len)
                        .await
                        .expect("domain len should read");
                    let mut domain = vec![0_u8; len[0] as usize];
                    sock.read_exact(&mut domain)
                        .await
                        .expect("domain should read");
                }
                _ => panic!("unexpected atyp {atyp}"),
            }
            let mut port = [0_u8; 2];
            sock.read_exact(&mut port).await.expect("port should read");

            // Success response
            sock.write_all(&[0x05, 0x00, 0x00, 0x01, 0, 0, 0, 0, 0, 0])
                .await
                .expect("connect response should write");

            let mut payload = vec![0_u8; 5];
            sock.read_exact(&mut payload)
                .await
                .expect("payload should read");
            let _ = payload_tx.send(payload);
        });

        let mut adapter = TorSocksAdapter::connect(TorSocksAdapterConfig {
            socks_proxy_addr: proxy_addr.to_string(),
            connect_timeout: Duration::from_secs(2),
            send_timeout: Duration::from_secs(2),
            outbound_queue_capacity: 16,
            max_payload_hint: None,
        })
        .expect("adapter should initialize");

        adapter
            .send(&"example.com:443".to_string(), b"hello")
            .expect("send should queue");
        let payload = tokio::time::timeout(Duration::from_secs(2), payload_rx)
            .await
            .expect("payload should arrive")
            .expect("payload channel should send");
        assert_eq!(payload, b"hello".to_vec());
        let metrics = adapter.metrics_snapshot();
        assert!(metrics.outbound_queued >= 1);
        assert!(metrics.send_attempts >= 1);
        assert!(metrics.send_success >= 1);

        server.await.expect("proxy task should complete");
    }

    #[test]
    fn rejects_invalid_peer_format() {
        let mut adapter = TorSocksAdapter::connect(TorSocksAdapterConfig::new("127.0.0.1:9050"))
            .expect("adapter should initialize");
        let err = adapter
            .send(&"not-a-peer".to_string(), b"hello")
            .expect_err("invalid peer should be rejected");
        assert!(err.to_string().contains("invalid peer format"));
    }
}
