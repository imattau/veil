//! QUIC transport adapter for VEIL fast-lane delivery.
//!
//! The adapter uses QUIC unidirectional streams for opaque byte payloads and
//! supports both outbound sends and inbound receive callbacks.

use std::net::{SocketAddr, ToSocketAddrs};
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{mpsc, Arc};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use quinn::{ClientConfig, Endpoint, ServerConfig};
use rustls::RootCertStore;
use rustls_pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use thiserror::Error;
use tokio::sync::{mpsc as tokio_mpsc, oneshot};
use veil_transport::adapter::{TransportAdapter, TransportHealthSnapshot};

#[derive(Debug, Clone)]
pub struct QuicIdentity {
    pub cert_der: Vec<u8>,
    pub key_der: Vec<u8>,
}

impl QuicIdentity {
    pub fn generate_self_signed(server_name: &str) -> Result<Self, QuicAdapterError> {
        let mut params = rcgen::CertificateParams::new(vec![server_name.to_string()])
            .map_err(|_| QuicAdapterError::IdentityGenerationFailed)?;
        params.is_ca = rcgen::IsCa::NoCa;
        let key_pair =
            rcgen::KeyPair::generate().map_err(|_| QuicAdapterError::IdentityGenerationFailed)?;
        let cert = params
            .self_signed(&key_pair)
            .map_err(|_| QuicAdapterError::IdentityGenerationFailed)?;
        let cert_der = cert.der().to_vec();
        let key_der = key_pair.serialize_der();
        Ok(Self { cert_der, key_der })
    }
}

#[derive(Debug, Clone)]
pub struct QuicAdapterConfig {
    pub bind_addr: SocketAddr,
    pub server_name: String,
    pub identity: QuicIdentity,
    pub trusted_peer_certs_der: Vec<Vec<u8>>,
    pub connect_timeout: Duration,
    pub send_timeout: Duration,
    pub outbound_queue_capacity: usize,
    pub inbound_queue_capacity: usize,
    pub max_recv_bytes: usize,
    pub max_payload_hint: Option<usize>,
}

impl QuicAdapterConfig {
    pub fn new(
        bind_addr: SocketAddr,
        server_name: impl Into<String>,
        identity: QuicIdentity,
    ) -> Self {
        Self {
            bind_addr,
            server_name: server_name.into(),
            trusted_peer_certs_der: Vec::new(),
            identity,
            connect_timeout: Duration::from_secs(3),
            send_timeout: Duration::from_secs(3),
            outbound_queue_capacity: 2048,
            inbound_queue_capacity: 4096,
            max_recv_bytes: 128 * 1024,
            max_payload_hint: Some(64 * 1024),
        }
    }
}

#[derive(Debug, Error)]
pub enum QuicAdapterError {
    #[error("adapter is closed")]
    Closed,
    #[error("outbound queue is full")]
    QueueFull,
    #[error("invalid peer address; expected socket address")]
    InvalidPeer,
    #[error("payload exceeds max payload hint ({hint} bytes)")]
    PayloadTooLarge { hint: usize },
    #[error("invalid certificate/key material")]
    InvalidIdentity,
    #[error("failed to generate identity")]
    IdentityGenerationFailed,
}

#[derive(Debug)]
struct OutboundMessage {
    peer: SocketAddr,
    bytes: Vec<u8>,
}

pub struct QuicAdapter {
    local_addr: SocketAddr,
    max_payload_hint: Option<usize>,
    outbound_tx: tokio_mpsc::Sender<OutboundMessage>,
    inbound_rx: mpsc::Receiver<(String, Vec<u8>)>,
    shutdown_tx: Option<oneshot::Sender<()>>,
    worker: Option<JoinHandle<()>>,
    running: Arc<AtomicBool>,
    metrics: Arc<QuicAdapterMetricsInner>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct QuicAdapterMetrics {
    pub outbound_queued: u64,
    pub send_attempts: u64,
    pub send_success: u64,
    pub send_errors: u64,
    pub inbound_received: u64,
    pub inbound_dropped: u64,
}

#[derive(Debug, Default)]
struct QuicAdapterMetricsInner {
    outbound_queued: AtomicU64,
    send_attempts: AtomicU64,
    send_success: AtomicU64,
    send_errors: AtomicU64,
    inbound_received: AtomicU64,
    inbound_dropped: AtomicU64,
}

impl QuicAdapter {
    pub fn connect(config: QuicAdapterConfig) -> Result<Self, QuicAdapterError> {
        let _ = rustls::crypto::ring::default_provider().install_default();

        let (startup_tx, startup_rx) = mpsc::sync_channel::<Result<(), QuicAdapterError>>(1);
        let (outbound_tx, outbound_rx) =
            tokio_mpsc::channel::<OutboundMessage>(config.outbound_queue_capacity);
        let (inbound_tx, inbound_rx) =
            mpsc::sync_channel::<(String, Vec<u8>)>(config.inbound_queue_capacity);
        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
        let running = Arc::new(AtomicBool::new(true));
        let metrics = Arc::new(QuicAdapterMetricsInner::default());
        let worker_running = Arc::clone(&running);
        let worker_metrics = Arc::clone(&metrics);
        let bind_addr = config.bind_addr;
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
            runtime.block_on(run_quic_worker(
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
            Ok(Err(err)) => return Err(err),
            Err(_) => return Err(QuicAdapterError::Closed),
        }

        Ok(Self {
            local_addr: bind_addr,
            max_payload_hint: config.max_payload_hint,
            outbound_tx,
            inbound_rx,
            shutdown_tx: Some(shutdown_tx),
            worker: Some(worker),
            running,
            metrics,
        })
    }

    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    pub fn metrics_snapshot(&self) -> QuicAdapterMetrics {
        QuicAdapterMetrics {
            outbound_queued: self.metrics.outbound_queued.load(Ordering::Relaxed),
            send_attempts: self.metrics.send_attempts.load(Ordering::Relaxed),
            send_success: self.metrics.send_success.load(Ordering::Relaxed),
            send_errors: self.metrics.send_errors.load(Ordering::Relaxed),
            inbound_received: self.metrics.inbound_received.load(Ordering::Relaxed),
            inbound_dropped: self.metrics.inbound_dropped.load(Ordering::Relaxed),
        }
    }
}

impl Drop for QuicAdapter {
    fn drop(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

impl TransportAdapter for QuicAdapter {
    type Peer = String;
    type Error = QuicAdapterError;

    fn send(&mut self, peer: &Self::Peer, bytes: &[u8]) -> Result<(), Self::Error> {
        if let Some(hint) = self.max_payload_hint {
            if bytes.len() > hint {
                return Err(QuicAdapterError::PayloadTooLarge { hint });
            }
        }
        let peer_addr = resolve_peer_addr(peer).map_err(|_| QuicAdapterError::InvalidPeer)?;
        self.outbound_tx
            .try_send(OutboundMessage {
                peer: peer_addr,
                bytes: bytes.to_vec(),
            })
            .map_err(|err| match err {
                tokio_mpsc::error::TrySendError::Full(_) => QuicAdapterError::QueueFull,
                tokio_mpsc::error::TrySendError::Closed(_) => QuicAdapterError::Closed,
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

    fn health_snapshot(&self) -> TransportHealthSnapshot {
        let m = self.metrics_snapshot();
        TransportHealthSnapshot {
            outbound_queued: m.outbound_queued,
            outbound_send_ok: m.send_success,
            outbound_send_err: m.send_errors,
            inbound_received: m.inbound_received,
            inbound_dropped: m.inbound_dropped,
            reconnect_attempts: 0,
        }
    }
}

async fn run_quic_worker(
    config: QuicAdapterConfig,
    running: Arc<AtomicBool>,
    metrics: Arc<QuicAdapterMetricsInner>,
    mut outbound_rx: tokio_mpsc::Receiver<OutboundMessage>,
    inbound_tx: mpsc::SyncSender<(String, Vec<u8>)>,
    mut shutdown_rx: oneshot::Receiver<()>,
    startup_tx: mpsc::SyncSender<Result<(), QuicAdapterError>>,
) {
    let debug = std::env::var_os("VEIL_QUIC_DEBUG").is_some();
    let server_cfg = match build_server_config(&config.identity) {
        Ok(cfg) => cfg,
        Err(_) => {
            running.store(false, Ordering::Relaxed);
            let _ = startup_tx.send(Err(QuicAdapterError::InvalidIdentity));
            return;
        }
    };

    let mut endpoint = match Endpoint::server(server_cfg, config.bind_addr) {
        Ok(ep) => ep,
        Err(_) => {
            running.store(false, Ordering::Relaxed);
            let _ = startup_tx.send(Err(QuicAdapterError::Closed));
            return;
        }
    };

    let client_cfg = match build_client_config(&config.trusted_peer_certs_der) {
        Ok(cfg) => cfg,
        Err(_) => {
            running.store(false, Ordering::Relaxed);
            let _ = startup_tx.send(Err(QuicAdapterError::InvalidIdentity));
            return;
        }
    };
    endpoint.set_default_client_config(client_cfg);
    let _ = startup_tx.send(Ok(()));

    loop {
        tokio::select! {
            _ = &mut shutdown_rx => break,
            Some(msg) = outbound_rx.recv() => {
                metrics.send_attempts.fetch_add(1, Ordering::Relaxed);
                if debug {
                    eprintln!("quic connecting to {}", msg.peer);
                }
                let connecting = endpoint.connect(msg.peer, &config.server_name);
                if let Ok(connecting) = connecting {
                    let connection = tokio::time::timeout(config.connect_timeout, connecting).await;
                    if let Ok(Ok(conn)) = connection {
                        let send_task = async {
                            let mut stream = conn.open_uni().await?;
                            stream.write_all(&msg.bytes).await?;
                            stream.finish()?;
                            let _ = stream.stopped().await;
                            Result::<(), quinn::WriteError>::Ok(())
                        };
                        let sent = tokio::time::timeout(config.send_timeout, send_task).await;
                        if matches!(sent, Ok(Ok(()))) {
                            metrics.send_success.fetch_add(1, Ordering::Relaxed);
                        } else {
                            metrics.send_errors.fetch_add(1, Ordering::Relaxed);
                            if debug {
                                match sent {
                                    Ok(Err(err)) => {
                                        eprintln!("quic send error: {err}");
                                    }
                                    Err(err) => {
                                        eprintln!("quic send timeout: {err}");
                                    }
                                    _ => {}
                                }
                            }
                        }
                    } else {
                        metrics.send_errors.fetch_add(1, Ordering::Relaxed);
                        if debug {
                            match connection {
                                Ok(Err(err)) => {
                                    eprintln!("quic connect error: {err}");
                                }
                                Err(err) => {
                                    eprintln!("quic connect timeout: {err}");
                                }
                                _ => {}
                            }
                        }
                    }
                } else {
                    metrics.send_errors.fetch_add(1, Ordering::Relaxed);
                    if debug {
                        eprintln!("quic connect builder error");
                    }
                }
            }
            maybe_incoming = endpoint.accept() => {
                if let Some(incoming) = maybe_incoming {
                    let inbound_tx = inbound_tx.clone();
                    let metrics = Arc::clone(&metrics);
                    let max_recv = config.max_recv_bytes;
                    if debug {
                        eprintln!("quic incoming connection");
                    }
                    tokio::spawn(async move {
                        match incoming.await {
                            Ok(conn) => {
                                let remote = conn.remote_address().to_string();
                                if debug {
                                    eprintln!("quic accepted from {remote}");
                                }
                                while let Ok(mut recv) = conn.accept_uni().await {
                                    match recv.read_to_end(max_recv).await {
                                        Ok(bytes) => {
                                            if debug {
                                                eprintln!("quic recv {} bytes from {remote}", bytes.len());
                                            }
                                            match inbound_tx.try_send((remote.clone(), bytes)) {
                                                Ok(_) => {
                                                    metrics.inbound_received.fetch_add(1, Ordering::Relaxed);
                                                }
                                                Err(_) => {
                                                    metrics.inbound_dropped.fetch_add(1, Ordering::Relaxed);
                                                }
                                            }
                                        }
                                        Err(err) => {
                                            if debug {
                                                eprintln!("quic recv error from {remote}: {err}");
                                            }
                                            break;
                                        }
                                    }
                                }
                            }
                            Err(err) => {
                                if debug {
                                    eprintln!("quic incoming connection failed: {err}");
                                }
                            }
                        }
                    });
                }
            }
        }
    }

    running.store(false, Ordering::Relaxed);
}

fn build_server_config(identity: &QuicIdentity) -> Result<ServerConfig, QuicAdapterError> {
    let cert = CertificateDer::from(identity.cert_der.clone());
    let key = PrivateKeyDer::from(PrivatePkcs8KeyDer::from(identity.key_der.clone()));
    ServerConfig::with_single_cert(vec![cert], key).map_err(|_| QuicAdapterError::InvalidIdentity)
}

fn build_client_config(trusted_certs_der: &[Vec<u8>]) -> Result<ClientConfig, QuicAdapterError> {
    if std::env::var_os("VEIL_QUIC_INSECURE").is_some() {
        return build_insecure_client_config();
    }
    if !trusted_certs_der.is_empty() {
        return build_pinned_client_config(trusted_certs_der);
    }
    let mut roots = RootCertStore::empty();
    roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    let tls = rustls::ClientConfig::builder()
        .with_root_certificates(roots)
        .with_no_client_auth();
    let quic_tls = quinn::crypto::rustls::QuicClientConfig::try_from(tls)
        .map_err(|_| QuicAdapterError::InvalidIdentity)?;
    Ok(ClientConfig::new(Arc::new(quic_tls)))
}

fn build_pinned_client_config(trusted_certs_der: &[Vec<u8>]) -> Result<ClientConfig, QuicAdapterError> {
    #[derive(Debug)]
    struct PinnedVerifier {
        pinned: Vec<Vec<u8>>,
    }

    impl rustls::client::danger::ServerCertVerifier for PinnedVerifier {
        fn verify_server_cert(
            &self,
            end_entity: &rustls_pki_types::CertificateDer<'_>,
            _intermediates: &[rustls_pki_types::CertificateDer<'_>],
            _server_name: &rustls::pki_types::ServerName<'_>,
            _ocsp: &[u8],
            _now: rustls::pki_types::UnixTime,
        ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
            if self.pinned.iter().any(|cert| cert.as_slice() == end_entity.as_ref()) {
                Ok(rustls::client::danger::ServerCertVerified::assertion())
            } else {
                Err(rustls::Error::InvalidCertificate(
                    rustls::CertificateError::UnknownIssuer,
                ))
            }
        }

        fn verify_tls12_signature(
            &self,
            _message: &[u8],
            _cert: &rustls_pki_types::CertificateDer<'_>,
            _dss: &rustls::DigitallySignedStruct,
        ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
            Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
        }

        fn verify_tls13_signature(
            &self,
            _message: &[u8],
            _cert: &rustls_pki_types::CertificateDer<'_>,
            _dss: &rustls::DigitallySignedStruct,
        ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
            Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
        }

        fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
            vec![
                rustls::SignatureScheme::RSA_PSS_SHA256,
                rustls::SignatureScheme::RSA_PSS_SHA384,
                rustls::SignatureScheme::RSA_PSS_SHA512,
                rustls::SignatureScheme::ECDSA_NISTP256_SHA256,
                rustls::SignatureScheme::ECDSA_NISTP384_SHA384,
                rustls::SignatureScheme::ED25519,
            ]
        }
    }

    let verifier = PinnedVerifier {
        pinned: trusted_certs_der.to_vec(),
    };
    let tls = rustls::ClientConfig::builder()
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(verifier))
        .with_no_client_auth();
    let quic_tls = quinn::crypto::rustls::QuicClientConfig::try_from(tls)
        .map_err(|_| QuicAdapterError::InvalidIdentity)?;
    Ok(ClientConfig::new(Arc::new(quic_tls)))
}

fn build_insecure_client_config() -> Result<ClientConfig, QuicAdapterError> {
    #[derive(Debug)]
    struct AcceptAllVerifier;
    impl rustls::client::danger::ServerCertVerifier for AcceptAllVerifier {
        fn verify_server_cert(
            &self,
            _end_entity: &rustls_pki_types::CertificateDer<'_>,
            _intermediates: &[rustls_pki_types::CertificateDer<'_>],
            _server_name: &rustls::pki_types::ServerName<'_>,
            _ocsp: &[u8],
            _now: rustls::pki_types::UnixTime,
        ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
            Ok(rustls::client::danger::ServerCertVerified::assertion())
        }

        fn verify_tls12_signature(
            &self,
            _message: &[u8],
            _cert: &rustls_pki_types::CertificateDer<'_>,
            _dss: &rustls::DigitallySignedStruct,
        ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
            Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
        }

        fn verify_tls13_signature(
            &self,
            _message: &[u8],
            _cert: &rustls_pki_types::CertificateDer<'_>,
            _dss: &rustls::DigitallySignedStruct,
        ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
            Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
        }

        fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
            vec![
                rustls::SignatureScheme::RSA_PSS_SHA256,
                rustls::SignatureScheme::RSA_PSS_SHA384,
                rustls::SignatureScheme::RSA_PSS_SHA512,
                rustls::SignatureScheme::ECDSA_NISTP256_SHA256,
                rustls::SignatureScheme::ECDSA_NISTP384_SHA384,
                rustls::SignatureScheme::ED25519,
            ]
        }
    }

    let tls = rustls::ClientConfig::builder()
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(AcceptAllVerifier))
        .with_no_client_auth();
    let quic_tls = quinn::crypto::rustls::QuicClientConfig::try_from(tls)
        .map_err(|_| QuicAdapterError::InvalidIdentity)?;
    Ok(ClientConfig::new(Arc::new(quic_tls)))
}

fn resolve_peer_addr(peer: &str) -> Result<SocketAddr, QuicAdapterError> {
    if let Ok(addr) = SocketAddr::from_str(peer) {
        return Ok(addr);
    }
    let mut addrs = peer
        .to_socket_addrs()
        .map_err(|_| QuicAdapterError::InvalidPeer)?;
    addrs.next().ok_or(QuicAdapterError::InvalidPeer)
}

#[cfg(test)]
mod tests {
    use super::{QuicAdapter, QuicAdapterConfig, QuicIdentity};
    use std::net::UdpSocket;
    use std::time::Duration;
    use veil_transport::adapter::TransportAdapter;

    fn free_udp_addr() -> std::net::SocketAddr {
        let sock = UdpSocket::bind("127.0.0.1:0").expect("bind should work");
        sock.local_addr().expect("local addr should resolve")
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn quic_adapter_initializes_and_queues_send() {
        let identity = QuicIdentity::generate_self_signed("localhost")
            .expect("identity generation should work");
        let addr_a = free_udp_addr();
        let _addr_b = free_udp_addr();

        let mut a = QuicAdapter::connect(QuicAdapterConfig::new(
            addr_a,
            "localhost",
            identity.clone(),
        ))
        .expect("adapter a should initialize");
        let _b = QuicAdapter::connect(QuicAdapterConfig::new(
            free_udp_addr(),
            "localhost",
            identity,
        ))
        .expect("adapter b should initialize");

        a.send(&"127.0.0.1:9".to_string(), b"ping")
            .expect("send should queue");
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert!(a.can_send());
        let metrics = a.metrics_snapshot();
        assert!(metrics.outbound_queued >= 1);
        assert!(metrics.send_attempts >= 1);
    }

    #[test]
    fn rejects_invalid_peer() {
        let identity = QuicIdentity::generate_self_signed("localhost")
            .expect("identity generation should work");
        let addr = free_udp_addr();
        let mut adapter = QuicAdapter::connect(QuicAdapterConfig::new(addr, "localhost", identity))
            .expect("adapter should initialize");
        let err = adapter
            .send(&"not-a-peer".to_string(), b"x")
            .expect_err("invalid peer should fail");
        assert!(err.to_string().contains("invalid peer"));
    }
}
