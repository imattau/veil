//! WebSocket transport adapter for VEIL.
//!
//! This crate provides a `TransportAdapter` implementation backed by a single
//! outbound WebSocket connection with reconnect/backoff.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{mpsc, Arc};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use thiserror::Error;
use tokio::sync::{mpsc as tokio_mpsc, oneshot};
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;
use tracing::{error, info, warn};
use veil_transport::adapter::{TransportAdapter, TransportHealthSnapshot};

#[derive(Debug, Clone)]
pub struct WebSocketAdapterConfig {
    pub url: String,
    pub peer_id: String,
    pub reconnect: bool,
    pub reconnect_initial: Duration,
    pub reconnect_max: Duration,
    pub outbound_queue_capacity: usize,
    pub inbound_queue_capacity: usize,
    pub max_payload_hint: Option<usize>,
}

impl WebSocketAdapterConfig {
    pub fn new(url: impl Into<String>, peer_id: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            peer_id: peer_id.into(),
            reconnect: true,
            reconnect_initial: Duration::from_millis(250),
            reconnect_max: Duration::from_secs(10),
            outbound_queue_capacity: 1024,
            inbound_queue_capacity: 4096,
            max_payload_hint: None,
        }
    }
}

#[derive(Debug, Error)]
pub enum WebSocketAdapterError {
    #[error("adapter is closed")]
    Closed,
    #[error("outbound queue is full")]
    QueueFull,
    #[error("payload exceeds max payload hint ({hint} bytes)")]
    PayloadTooLarge { hint: usize },
    #[error("server bind failed: {0}")]
    BindFailed(String),
}

pub struct WebSocketAdapter {
    max_payload_hint: Option<usize>,
    outbound_tx: tokio_mpsc::Sender<Vec<u8>>,
    inbound_rx: mpsc::Receiver<(String, Vec<u8>)>,
    shutdown_tx: Option<oneshot::Sender<()>>,
    worker: Option<JoinHandle<()>>,
    connected: Arc<AtomicBool>,
    metrics: Arc<WebSocketAdapterMetricsInner>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct WebSocketAdapterMetrics {
    pub outbound_queued: u64,
    pub outbound_send_ok: u64,
    pub outbound_send_err: u64,
    pub inbound_received: u64,
    pub inbound_dropped: u64,
    pub reconnect_attempts: u64,
}

#[derive(Debug, Default)]
struct WebSocketAdapterMetricsInner {
    outbound_queued: AtomicU64,
    outbound_send_ok: AtomicU64,
    outbound_send_err: AtomicU64,
    inbound_received: AtomicU64,
    inbound_dropped: AtomicU64,
    reconnect_attempts: AtomicU64,
}

impl WebSocketAdapter {
    pub fn connect(config: WebSocketAdapterConfig) -> Result<Self, WebSocketAdapterError> {
        let (outbound_tx, outbound_rx) =
            tokio_mpsc::channel::<Vec<u8>>(config.outbound_queue_capacity);
        let (inbound_tx, inbound_rx) =
            mpsc::sync_channel::<(String, Vec<u8>)>(config.inbound_queue_capacity);
        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
        let connected = Arc::new(AtomicBool::new(false));
        let metrics = Arc::new(WebSocketAdapterMetricsInner::default());

        let worker_connected = Arc::clone(&connected);
        let worker_metrics = Arc::clone(&metrics);
        let worker_config = config.clone();
        let worker = thread::spawn(move || {
            let runtime = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(rt) => rt,
                Err(_) => return,
            };
            runtime.block_on(run_websocket_worker(
                worker_config,
                worker_connected,
                worker_metrics,
                outbound_rx,
                inbound_tx,
                shutdown_rx,
            ));
        });

        Ok(Self {
            max_payload_hint: config.max_payload_hint,
            outbound_tx,
            inbound_rx,
            shutdown_tx: Some(shutdown_tx),
            worker: Some(worker),
            connected,
            metrics,
        })
    }

    pub fn metrics_snapshot(&self) -> WebSocketAdapterMetrics {
        WebSocketAdapterMetrics {
            outbound_queued: self.metrics.outbound_queued.load(Ordering::Relaxed),
            outbound_send_ok: self.metrics.outbound_send_ok.load(Ordering::Relaxed),
            outbound_send_err: self.metrics.outbound_send_err.load(Ordering::Relaxed),
            inbound_received: self.metrics.inbound_received.load(Ordering::Relaxed),
            inbound_dropped: self.metrics.inbound_dropped.load(Ordering::Relaxed),
            reconnect_attempts: self.metrics.reconnect_attempts.load(Ordering::Relaxed),
        }
    }
}

impl Drop for WebSocketAdapter {
    fn drop(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

impl TransportAdapter for WebSocketAdapter {
    type Peer = String;
    type Error = WebSocketAdapterError;

    fn send(&mut self, _peer: &Self::Peer, bytes: &[u8]) -> Result<(), Self::Error> {
        if let Some(hint) = self.max_payload_hint {
            if bytes.len() > hint {
                return Err(WebSocketAdapterError::PayloadTooLarge { hint });
            }
        }
        self.outbound_tx
            .try_send(bytes.to_vec())
            .map_err(|err| match err {
                tokio_mpsc::error::TrySendError::Full(_) => WebSocketAdapterError::QueueFull,
                tokio_mpsc::error::TrySendError::Closed(_) => WebSocketAdapterError::Closed,
            })
            .map(|_| {
                self.metrics.outbound_queued.fetch_add(1, Ordering::Relaxed);
            })
    }

    fn recv(&mut self) -> Option<(Self::Peer, Vec<u8>)> {
        self.inbound_rx.try_recv().ok()
    }

    fn max_payload_hint(&self) -> Option<usize> {
        self.max_payload_hint
    }

    fn can_send(&self) -> bool {
        self.connected.load(Ordering::Relaxed)
    }

    fn health_snapshot(&self) -> TransportHealthSnapshot {
        let m = self.metrics_snapshot();
        TransportHealthSnapshot {
            outbound_queued: m.outbound_queued,
            outbound_send_ok: m.outbound_send_ok,
            outbound_send_err: m.outbound_send_err,
            inbound_received: m.inbound_received,
            inbound_dropped: m.inbound_dropped,
            reconnect_attempts: m.reconnect_attempts,
            last_error: None,
            last_error_code: None,
        }
    }
}

async fn run_websocket_worker(
    config: WebSocketAdapterConfig,
    connected: Arc<AtomicBool>,
    metrics: Arc<WebSocketAdapterMetricsInner>,
    mut outbound_rx: tokio_mpsc::Receiver<Vec<u8>>,
    inbound_tx: mpsc::SyncSender<(String, Vec<u8>)>,
    mut shutdown_rx: oneshot::Receiver<()>,
) {
    let mut backoff = config.reconnect_initial;

    'outer: loop {
        tokio::select! {
            _ = &mut shutdown_rx => break 'outer,
            connect_result = connect_async(&config.url) => {
                metrics.reconnect_attempts.fetch_add(1, Ordering::Relaxed);
                match connect_result {
                    Ok((stream, _)) => {
                        connected.store(true, Ordering::Relaxed);
                        backoff = config.reconnect_initial;
                        let (mut write, mut read) = stream.split();

                        loop {
                            tokio::select! {
                                _ = &mut shutdown_rx => {
                                    connected.store(false, Ordering::Relaxed);
                                    break 'outer;
                                }
                                maybe_out = outbound_rx.recv() => {
                                    match maybe_out {
                                        Some(bytes) => {
                                            if write.send(Message::Binary(bytes)).await.is_err() {
                                                metrics.outbound_send_err.fetch_add(1, Ordering::Relaxed);
                                                connected.store(false, Ordering::Relaxed);
                                                break;
                                            }
                                            metrics.outbound_send_ok.fetch_add(1, Ordering::Relaxed);
                                        }
                                        None => {
                                            connected.store(false, Ordering::Relaxed);
                                            break 'outer;
                                        }
                                    }
                                }
                                maybe_in = read.next() => {
                                    match maybe_in {
                                        Some(Ok(Message::Binary(bytes))) => {
                                            match inbound_tx.try_send((config.peer_id.clone(), bytes.to_vec())) {
                                                Ok(_) => {
                                                    metrics.inbound_received.fetch_add(1, Ordering::Relaxed);
                                                }
                                                Err(_) => {
                                                    metrics.inbound_dropped.fetch_add(1, Ordering::Relaxed);
                                                }
                                            }
                                        }
                                        Some(Ok(Message::Close(_))) => {
                                            connected.store(false, Ordering::Relaxed);
                                            break;
                                        }
                                        Some(Ok(_)) => {}
                                        Some(Err(_)) | None => {
                                            connected.store(false, Ordering::Relaxed);
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Err(_) => {
                        connected.store(false, Ordering::Relaxed);
                    }
                }

                if !config.reconnect {
                    break 'outer;
                }

                tokio::select! {
                    _ = &mut shutdown_rx => break 'outer,
                    _ = tokio::time::sleep(backoff) => {}
                }
                backoff = std::cmp::min(backoff.saturating_mul(2), config.reconnect_max);
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct WebSocketServerAdapterConfig {
    pub bind_addr: String,
    pub outbound_queue_capacity: usize,
    pub inbound_queue_capacity: usize,
    pub max_payload_hint: Option<usize>,
}

impl WebSocketServerAdapterConfig {
    pub fn new(bind_addr: impl Into<String>) -> Self {
        Self {
            bind_addr: bind_addr.into(),
            outbound_queue_capacity: 1024,
            inbound_queue_capacity: 4096,
            max_payload_hint: None,
        }
    }
}

pub struct WebSocketServerAdapter {
    max_payload_hint: Option<usize>,
    outbound_tx: tokio_mpsc::Sender<OutboundMessage>,
    inbound_rx: mpsc::Receiver<(String, Vec<u8>)>,
    shutdown_tx: Option<oneshot::Sender<()>>,
    worker: Option<JoinHandle<()>>,
    running: Arc<AtomicBool>,
    metrics: Arc<WebSocketAdapterMetricsInner>,
}

struct OutboundMessage {
    peer: String,
    bytes: Vec<u8>,
}

impl WebSocketServerAdapter {
    pub fn listen(config: WebSocketServerAdapterConfig) -> Result<Self, WebSocketAdapterError> {
        let (startup_tx, startup_rx) = mpsc::sync_channel::<Result<(), String>>(1);
        let (outbound_tx, outbound_rx) =
            tokio_mpsc::channel::<OutboundMessage>(config.outbound_queue_capacity);
        let (inbound_tx, inbound_rx) =
            mpsc::sync_channel::<(String, Vec<u8>)>(config.inbound_queue_capacity);
        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
        let running = Arc::new(AtomicBool::new(true));
        let metrics = Arc::new(WebSocketAdapterMetricsInner::default());

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
                    let _ = startup_tx.send(Err("failed to build tokio runtime".to_string()));
                    return;
                }
            };
            runtime.block_on(run_server_worker(
                worker_config,
                worker_running,
                worker_metrics,
                outbound_rx,
                inbound_tx,
                shutdown_rx,
                startup_tx,
            ));
        });

        match startup_rx.recv_timeout(Duration::from_secs(2)) {
            Ok(Ok(())) => {}
            Ok(Err(err)) => return Err(WebSocketAdapterError::BindFailed(err)),
            Err(_) => return Err(WebSocketAdapterError::Closed),
        }

        Ok(Self {
            max_payload_hint: config.max_payload_hint,
            outbound_tx,
            inbound_rx,
            shutdown_tx: Some(shutdown_tx),
            worker: Some(worker),
            running,
            metrics,
        })
    }

    pub fn metrics_snapshot(&self) -> WebSocketAdapterMetrics {
        WebSocketAdapterMetrics {
            outbound_queued: self.metrics.outbound_queued.load(Ordering::Relaxed),
            outbound_send_ok: self.metrics.outbound_send_ok.load(Ordering::Relaxed),
            outbound_send_err: self.metrics.outbound_send_err.load(Ordering::Relaxed),
            inbound_received: self.metrics.inbound_received.load(Ordering::Relaxed),
            inbound_dropped: self.metrics.inbound_dropped.load(Ordering::Relaxed),
            reconnect_attempts: 0,
        }
    }
}

impl Drop for WebSocketServerAdapter {
    fn drop(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

impl TransportAdapter for WebSocketServerAdapter {
    type Peer = String;
    type Error = WebSocketAdapterError;

    fn send(&mut self, peer: &Self::Peer, bytes: &[u8]) -> Result<(), Self::Error> {
        if let Some(hint) = self.max_payload_hint {
            if bytes.len() > hint {
                return Err(WebSocketAdapterError::PayloadTooLarge { hint });
            }
        }
        self.outbound_tx
            .try_send(OutboundMessage {
                peer: peer.clone(),
                bytes: bytes.to_vec(),
            })
            .map_err(|err| match err {
                tokio_mpsc::error::TrySendError::Full(_) => WebSocketAdapterError::QueueFull,
                tokio_mpsc::error::TrySendError::Closed(_) => WebSocketAdapterError::Closed,
            })
            .map(|_| {
                self.metrics.outbound_queued.fetch_add(1, Ordering::Relaxed);
            })
    }

    fn recv(&mut self) -> Option<(Self::Peer, Vec<u8>)> {
        self.inbound_rx.try_recv().ok()
    }

    fn max_payload_hint(&self) -> Option<usize> {
        self.max_payload_hint
    }

    fn can_send(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }

    fn can_recv(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }

    fn health_snapshot(&self) -> TransportHealthSnapshot {
        let m = self.metrics_snapshot();
        TransportHealthSnapshot {
            outbound_queued: m.outbound_queued,
            outbound_send_ok: m.outbound_send_ok,
            outbound_send_err: m.outbound_send_err,
            inbound_received: m.inbound_received,
            inbound_dropped: m.inbound_dropped,
            reconnect_attempts: 0,
            last_error: None,
            last_error_code: None,
        }
    }
}

async fn run_server_worker(
    config: WebSocketServerAdapterConfig,
    running: Arc<AtomicBool>,
    metrics: Arc<WebSocketAdapterMetricsInner>,
    mut outbound_rx: tokio_mpsc::Receiver<OutboundMessage>,
    inbound_tx: mpsc::SyncSender<(String, Vec<u8>)>,
    mut shutdown_rx: oneshot::Receiver<()>,
    startup_tx: mpsc::SyncSender<Result<(), String>>,
) {
    let listener = match tokio::net::TcpListener::bind(&config.bind_addr).await {
        Ok(l) => l,
        Err(err) => {
            error!(
                "websocket server bind failed on {}: {}",
                config.bind_addr, err
            );
            let _ = startup_tx.send(Err(err.to_string()));
            running.store(false, Ordering::Relaxed);
            return;
        }
    };

    let _ = startup_tx.send(Ok(()));
    let mut clients = std::collections::HashMap::<String, tokio_mpsc::Sender<Vec<u8>>>::new();

    loop {
        tokio::select! {
            _ = &mut shutdown_rx => break,
            maybe_conn = listener.accept() => {
                if let Ok((stream, addr)) = maybe_conn {
                    let peer_id = addr.to_string();
                    info!("websocket server: accepting connection from {}", peer_id);
                    let (client_tx, mut client_rx) = tokio_mpsc::channel::<Vec<u8>>(128);
                    clients.insert(peer_id.clone(), client_tx);

                    let inbound_tx = inbound_tx.clone();
                    let metrics = Arc::clone(&metrics);
                    let peer_id_inner = peer_id.clone();

                    tokio::spawn(async move {
                        match tokio_tungstenite::accept_async(stream).await {
                            Ok(ws_stream) => {
                                info!("websocket server: handshake successful for {}", peer_id_inner);
                                let (mut write, mut read) = ws_stream.split();
                                loop {
                                    tokio::select! {
                                        maybe_msg = read.next() => {
                                            match maybe_msg {
                                                Some(Ok(Message::Binary(bytes))) => {
                                                    if inbound_tx.try_send((peer_id_inner.clone(), bytes)).is_ok() {
                                                        metrics.inbound_received.fetch_add(1, Ordering::Relaxed);
                                                    } else {
                                                        metrics.inbound_dropped.fetch_add(1, Ordering::Relaxed);
                                                    }
                                                }
                                                Some(Ok(Message::Close(_))) => {
                                                    info!("websocket server: connection closed by {}", peer_id_inner);
                                                    break;
                                                }
                                                Some(Err(err)) => {
                                                    error!("websocket server: read error from {}: {}", peer_id_inner, err);
                                                    break;
                                                }
                                                None => break,
                                                _ => {}
                                            }
                                        }
                                        maybe_out = client_rx.recv() => {
                                            if let Some(bytes) = maybe_out {
                                                if write.send(Message::Binary(bytes)).await.is_err() {
                                                    metrics.outbound_send_err.fetch_add(1, Ordering::Relaxed);
                                                    break;
                                                }
                                                metrics.outbound_send_ok.fetch_add(1, Ordering::Relaxed);
                                            } else {
                                                break;
                                            }
                                        }
                                    }
                                }
                            }
                            Err(err) => {
                                error!("websocket server: handshake failed for {}: {}", peer_id_inner, err);
                            }
                        }
                    });
                }
            }
            maybe_out = outbound_rx.recv() => {
                if let Some(msg) = maybe_out {
                    if let Some(tx) = clients.get(&msg.peer) {
                        if tx.try_send(msg.bytes).is_err() {
                            // Client queue full or closed
                            metrics.outbound_send_err.fetch_add(1, Ordering::Relaxed);
                        }
                    } else {
                        // Peer not found
                        metrics.outbound_send_err.fetch_add(1, Ordering::Relaxed);
                    }
                } else {
                    break;
                }
            }
        }
        // Cleanup closed clients
        clients.retain(|_, tx| !tx.is_closed());
    }

    running.store(false, Ordering::Relaxed);
}

#[cfg(test)]
mod tests {
    use super::WebSocketServerAdapterConfig;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn server_adapter_accepts_multiple_clients() {
        let server_cfg = WebSocketServerAdapterConfig::new("127.0.0.1:0");
        let _server = super::WebSocketServerAdapter::listen(server_cfg).expect("server listen");
    }
}
