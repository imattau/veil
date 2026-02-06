use std::collections::HashSet;
use std::env;
use std::fs;
use std::hash::Hash;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use rand::RngCore;
use veil_crypto::aead::XChaCha20Poly1305Cipher;
use veil_crypto::signing::Ed25519Verifier;
use veil_node::config::{AdaptiveLaneScoringConfig, NodeRuntimeConfig};
use veil_node::persistence::{load_state_or_default, save_state_to_path};
use veil_node::service::{NodeRuntime, NodeRuntimeCallbacks};
use veil_transport::adapter::{TransportAdapter, TransportHealthSnapshot};
use veil_transport_quic::{QuicAdapter, QuicAdapterConfig, QuicIdentity};
use veil_transport_tor::{TorSocksAdapter, TorSocksAdapterConfig};
use veil_transport_websocket::{WebSocketAdapter, WebSocketAdapterConfig};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum FallbackPeer {
    WebSocket(String),
    Tor(String),
}

impl std::fmt::Display for FallbackPeer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FallbackPeer::WebSocket(peer) => write!(f, "ws:{peer}"),
            FallbackPeer::Tor(peer) => write!(f, "tor:{peer}"),
        }
    }
}

#[derive(Debug)]
enum FallbackSendError {
    WebSocket,
    Tor,
    MissingWebSocket,
    MissingTor,
}

struct CombinedFallbackAdapter {
    ws: Option<WebSocketAdapter>,
    tor: Option<TorSocksAdapter>,
}

impl CombinedFallbackAdapter {
    fn new(ws: Option<WebSocketAdapter>, tor: Option<TorSocksAdapter>) -> Self {
        Self { ws, tor }
    }

    fn ws_mut(&mut self) -> Option<&mut WebSocketAdapter> {
        self.ws.as_mut()
    }

    fn tor_mut(&mut self) -> Option<&mut TorSocksAdapter> {
        self.tor.as_mut()
    }

    fn combined_max_payload_hint(&self) -> Option<usize> {
        let ws_hint = self.ws.as_ref().and_then(|w| w.max_payload_hint());
        let tor_hint = self.tor.as_ref().and_then(|t| t.max_payload_hint());
        match (ws_hint, tor_hint) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (Some(a), None) => Some(a),
            (None, Some(b)) => Some(b),
            (None, None) => None,
        }
    }

    fn combined_health_snapshot(&self) -> TransportHealthSnapshot {
        let mut out = TransportHealthSnapshot::default();
        if let Some(ws) = &self.ws {
            let h = ws.health_snapshot();
            out.outbound_queued += h.outbound_queued;
            out.outbound_send_ok += h.outbound_send_ok;
            out.outbound_send_err += h.outbound_send_err;
            out.inbound_received += h.inbound_received;
            out.inbound_dropped += h.inbound_dropped;
            out.reconnect_attempts += h.reconnect_attempts;
        }
        if let Some(tor) = &self.tor {
            let h = tor.health_snapshot();
            out.outbound_queued += h.outbound_queued;
            out.outbound_send_ok += h.outbound_send_ok;
            out.outbound_send_err += h.outbound_send_err;
            out.inbound_received += h.inbound_received;
            out.inbound_dropped += h.inbound_dropped;
            out.reconnect_attempts += h.reconnect_attempts;
        }
        out
    }
}

impl TransportAdapter for CombinedFallbackAdapter {
    type Peer = FallbackPeer;
    type Error = FallbackSendError;

    fn send(&mut self, peer: &Self::Peer, bytes: &[u8]) -> Result<(), Self::Error> {
        match peer {
            FallbackPeer::WebSocket(ws_peer) => {
                let ws = self.ws_mut().ok_or(FallbackSendError::MissingWebSocket)?;
                ws.send(ws_peer, bytes)
                    .map_err(|_| FallbackSendError::WebSocket)
            }
            FallbackPeer::Tor(tor_peer) => {
                let tor = self.tor_mut().ok_or(FallbackSendError::MissingTor)?;
                tor.send(tor_peer, bytes)
                    .map_err(|_| FallbackSendError::Tor)
            }
        }
    }

    fn recv(&mut self) -> Option<(Self::Peer, Vec<u8>)> {
        if let Some(ws) = self.ws_mut() {
            if let Some((peer, bytes)) = ws.recv() {
                return Some((FallbackPeer::WebSocket(peer), bytes));
            }
        }
        None
    }

    fn max_payload_hint(&self) -> Option<usize> {
        self.combined_max_payload_hint()
    }

    fn can_send(&self) -> bool {
        self.ws.as_ref().map(|w| w.can_send()).unwrap_or(false)
            || self.tor.as_ref().map(|t| t.can_send()).unwrap_or(false)
    }

    fn can_recv(&self) -> bool {
        self.ws.as_ref().map(|w| w.can_recv()).unwrap_or(false)
    }

    fn health_snapshot(&self) -> TransportHealthSnapshot {
        self.combined_health_snapshot()
    }
}

struct RecordingAdapter<A: TransportAdapter> {
    inner: A,
    seen: Arc<Mutex<HashSet<A::Peer>>>,
}

impl<A: TransportAdapter> RecordingAdapter<A> {
    fn new(inner: A, seen: Arc<Mutex<HashSet<A::Peer>>>) -> Self {
        Self { inner, seen }
    }

    fn snapshot_seen(&self) -> Vec<A::Peer>
    where
        A::Peer: Clone,
    {
        let guard = self.seen.lock().unwrap_or_else(|e| e.into_inner());
        guard.iter().cloned().collect()
    }
}

impl<A: TransportAdapter> TransportAdapter for RecordingAdapter<A>
where
    A::Peer: Clone + Eq + Hash,
{
    type Peer = A::Peer;
    type Error = A::Error;

    fn send(&mut self, peer: &Self::Peer, bytes: &[u8]) -> Result<(), Self::Error> {
        self.inner.send(peer, bytes)
    }

    fn recv(&mut self) -> Option<(Self::Peer, Vec<u8>)> {
        let item = self.inner.recv();
        if let Some((ref peer, _)) = item {
            let mut guard = self.seen.lock().unwrap_or_else(|e| e.into_inner());
            guard.insert(peer.clone());
        }
        item
    }

    fn max_payload_hint(&self) -> Option<usize> {
        self.inner.max_payload_hint()
    }

    fn can_send(&self) -> bool {
        self.inner.can_send()
    }

    fn can_recv(&self) -> bool {
        self.inner.can_recv()
    }

    fn health_snapshot(&self) -> TransportHealthSnapshot {
        self.inner.health_snapshot()
    }
}

fn env_var(key: &str, default: &str) -> String {
    env::var(key).unwrap_or_else(|_| default.to_string())
}

fn env_u64(key: &str, default: u64) -> u64 {
    env::var(key)
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(default)
}

fn env_usize(key: &str, default: usize) -> usize {
    env::var(key)
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(default)
}

fn env_bool(key: &str, default: bool) -> bool {
    env::var(key)
        .ok()
        .map(|value| matches!(value.as_str(), "1" | "true" | "TRUE" | "yes" | "YES"))
        .unwrap_or(default)
}

fn env_list(key: &str) -> Vec<String> {
    env::var(key)
        .ok()
        .map(|value| {
            value
                .split(',')
                .map(|entry| entry.trim())
                .filter(|entry| !entry.is_empty())
                .map(|entry| entry.to_string())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn load_peer_list(path: &Path) -> Vec<String> {
    if !path.exists() {
        return Vec::new();
    }
    let Ok(contents) = fs::read_to_string(path) else {
        return Vec::new();
    };
    contents
        .lines()
        .map(|line| line.trim())
        .filter(|line| !line.is_empty())
        .map(|line| line.to_string())
        .collect()
}

fn save_peer_list(path: &Path, peers: &[String]) {
    if let Err(err) = ensure_parent(path) {
        eprintln!("failed to create peer list dir: {err}");
        return;
    }
    let mut contents = String::new();
    for peer in peers {
        contents.push_str(peer);
        contents.push('\n');
    }
    if let Err(err) = fs::write(path, contents) {
        eprintln!("failed to write peer list: {err}");
    }
}

fn ensure_parent(path: &Path) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent)?;
        }
    }
    Ok(())
}

fn load_or_create_identity(cert_path: &Path, key_path: &Path) -> Result<QuicIdentity, String> {
    if cert_path.exists() && key_path.exists() {
        let cert = fs::read(cert_path).map_err(|e| format!("read cert: {e}"))?;
        let key = fs::read(key_path).map_err(|e| format!("read key: {e}"))?;
        return Ok(QuicIdentity {
            cert_der: cert,
            key_der: key,
        });
    }

    let identity = QuicIdentity::generate_self_signed("veil-node")
        .map_err(|e| format!("generate identity: {e}"))?;
    ensure_parent(cert_path).map_err(|e| format!("create cert dir: {e}"))?;
    ensure_parent(key_path).map_err(|e| format!("create key dir: {e}"))?;
    fs::write(cert_path, &identity.cert_der).map_err(|e| format!("write cert: {e}"))?;
    fs::write(key_path, &identity.key_der).map_err(|e| format!("write key: {e}"))?;
    Ok(identity)
}

fn load_trusted_certs(paths: &[String]) -> Vec<Vec<u8>> {
    let mut out = Vec::new();
    for path in paths {
        match fs::read(path) {
            Ok(bytes) => out.push(bytes),
            Err(err) => eprintln!("failed to read trusted cert {path}: {err}"),
        }
    }
    out
}

fn parse_required_signed_namespaces(values: &[String]) -> HashSet<u16> {
    let mut out = HashSet::new();
    for value in values {
        if let Ok(ns) = value.parse::<u16>() {
            out.insert(ns);
        }
    }
    out
}

fn decode_hex_tag_32(value: &str) -> Option<[u8; 32]> {
    if value.len() != 64 {
        return None;
    }
    let mut out = [0_u8; 32];
    for (idx, chunk) in value.as_bytes().chunks_exact(2).enumerate() {
        let s = std::str::from_utf8(chunk).ok()?;
        let byte = u8::from_str_radix(s, 16).ok()?;
        out[idx] = byte;
    }
    Some(out)
}

fn parse_core_tags(values: &[String]) -> Vec<[u8; 32]> {
    values
        .iter()
        .filter_map(|value| decode_hex_tag_32(value))
        .collect()
}

fn parse_fallback_peers(ws_peer: Option<String>, tor_peers: Vec<String>) -> Vec<FallbackPeer> {
    let mut peers = Vec::new();
    if let Some(ws_peer) = ws_peer {
        peers.push(FallbackPeer::WebSocket(ws_peer));
    }
    for peer in tor_peers {
        peers.push(FallbackPeer::Tor(peer));
    }
    peers
}

fn parse_fallback_peer_strings(values: &[String]) -> Vec<FallbackPeer> {
    values
        .iter()
        .filter_map(|value| {
            if let Some(rest) = value.strip_prefix("ws:") {
                Some(FallbackPeer::WebSocket(rest.to_string()))
            } else if let Some(rest) = value.strip_prefix("tor:") {
                Some(FallbackPeer::Tor(rest.to_string()))
            } else {
                None
            }
        })
        .collect()
}

fn encode_fallback_peers(peers: &[FallbackPeer]) -> Vec<String> {
    peers.iter().map(|peer| peer.to_string()).collect()
}

fn merge_peers<T: Clone + Eq + Hash>(
    configured: &[T],
    discovered: &[T],
    max_total: usize,
) -> Vec<T> {
    let mut out = Vec::new();
    for peer in configured {
        if !out.contains(peer) {
            out.push(peer.clone());
        }
    }
    for peer in discovered {
        if out.len() >= max_total {
            break;
        }
        if !out.contains(peer) {
            out.push(peer.clone());
        }
    }
    out
}

fn load_or_create_node_key(path: &Path) -> Result<[u8; 32], String> {
    if path.exists() {
        let bytes = fs::read(path).map_err(|e| format!("read node key: {e}"))?;
        if bytes.len() == 32 {
            let mut out = [0_u8; 32];
            out.copy_from_slice(&bytes);
            return Ok(out);
        }
    }

    let mut key = [0_u8; 32];
    rand::thread_rng().fill_bytes(&mut key);
    ensure_parent(path).map_err(|e| format!("create node key dir: {e}"))?;
    fs::write(path, key).map_err(|e| format!("write node key: {e}"))?;
    Ok(key)
}

#[derive(Debug, Default)]
struct MetricsState {
    ticks: AtomicU64,
    delivered: AtomicU64,
    send_failures: AtomicU64,
    ack_clears: AtomicU64,
    last_fast_outbound_ok: AtomicU64,
    last_fast_outbound_err: AtomicU64,
    last_fallback_outbound_ok: AtomicU64,
    last_fallback_outbound_err: AtomicU64,
    last_fast_inbound: AtomicU64,
    last_fallback_inbound: AtomicU64,
}

fn start_health_server(port: u16, metrics: Arc<MetricsState>) {
    if port == 0 {
        return;
    }
    thread::spawn(move || {
        let listener = match TcpListener::bind(("127.0.0.1", port)) {
            Ok(listener) => listener,
            Err(err) => {
                eprintln!("health server bind failed on {port}: {err}");
                return;
            }
        };
        for stream in listener.incoming() {
            if let Ok(mut stream) = stream {
                let mut buf = [0_u8; 1024];
                let _ = stream.read(&mut buf);
                let req = String::from_utf8_lossy(&buf);
                let ok = req.starts_with("GET /health") || req.starts_with("GET /healthz");
                let is_metrics = req.starts_with("GET /metrics");
                let (status, body) = if ok {
                    ("200 OK", "ok".to_string())
                } else if is_metrics {
                    let body = format!(
                        "veil_ticks_total {}\nveil_delivered_total {}\nveil_send_failures_total {}\nveil_ack_clears_total {}\nveil_fast_outbound_ok {}\nveil_fast_outbound_err {}\nveil_fallback_outbound_ok {}\nveil_fallback_outbound_err {}\nveil_fast_inbound {}\nveil_fallback_inbound {}\n",
                        metrics.ticks.load(Ordering::Relaxed),
                        metrics.delivered.load(Ordering::Relaxed),
                        metrics.send_failures.load(Ordering::Relaxed),
                        metrics.ack_clears.load(Ordering::Relaxed),
                        metrics.last_fast_outbound_ok.load(Ordering::Relaxed),
                        metrics.last_fast_outbound_err.load(Ordering::Relaxed),
                        metrics.last_fallback_outbound_ok.load(Ordering::Relaxed),
                        metrics.last_fallback_outbound_err.load(Ordering::Relaxed),
                        metrics.last_fast_inbound.load(Ordering::Relaxed),
                        metrics.last_fallback_inbound.load(Ordering::Relaxed),
                    );
                    ("200 OK", body)
                } else {
                    ("404 Not Found", "not found".to_string())
                };
                let resp = format!(
                    "HTTP/1.1 {status}\r\nContent-Length: {}\r\nContent-Type: text/plain\r\n\r\n{body}",
                    body.len()
                );
                let _ = stream.write_all(resp.as_bytes());
            }
        }
    });
}

fn main() {
    let state_path = PathBuf::from(env_var(
        "VEIL_VPS_STATE_PATH",
        "data/veil-vps-node-state.cbor",
    ));
    let node_key_path = PathBuf::from(env_var("VEIL_VPS_NODE_KEY_PATH", "data/node_identity.key"));
    let quic_cert_path = PathBuf::from(env_var("VEIL_VPS_QUIC_CERT_PATH", "data/quic_cert.der"));
    let quic_key_path = PathBuf::from(env_var("VEIL_VPS_QUIC_KEY_PATH", "data/quic_key.der"));
    let snapshot_secs = env_u64("VEIL_VPS_SNAPSHOT_SECS", 60);
    let tick_ms = env_u64("VEIL_VPS_TICK_MS", 50);
    let health_port = env_u64("VEIL_VPS_HEALTH_PORT", 9090) as u16;
    let fast_peers = env_list("VEIL_VPS_FAST_PEERS");
    let core_tags = env_list("VEIL_VPS_CORE_TAGS");
    let tor_peers = env_list("VEIL_VPS_TOR_PEERS");
    let peer_list_path = PathBuf::from(env_var(
        "VEIL_VPS_PEER_LIST_PATH",
        "data/discovered_peers.txt",
    ));
    let max_dynamic_peers = env_usize("VEIL_VPS_MAX_DYNAMIC_PEERS", 512);

    let quic_bind = env_var("VEIL_VPS_QUIC_BIND", "0.0.0.0:5000");
    let ws_url = env::var("VEIL_VPS_WS_URL").ok();
    let ws_peer = env::var("VEIL_VPS_WS_PEER").ok();
    let ws_peer_id = ws_peer.clone().unwrap_or_else(|| "ws-peer".to_string());
    let tor_socks_addr = env::var("VEIL_VPS_TOR_SOCKS_ADDR").ok();

    let adaptive_scoring = env_bool("VEIL_VPS_ADAPTIVE_LANE_SCORING", true);
    let max_cache_shards = env_usize("VEIL_VPS_MAX_CACHE_SHARDS", 200_000);
    let bucket_jitter = env_usize("VEIL_VPS_BUCKET_JITTER", 0);
    let required_signed =
        parse_required_signed_namespaces(&env_list("VEIL_VPS_REQUIRED_SIGNED_NAMESPACES"));

    let node_key = match load_or_create_node_key(&node_key_path) {
        Ok(key) => key,
        Err(err) => {
            eprintln!("fatal: {err}");
            return;
        }
    };

    let identity = match load_or_create_identity(&quic_cert_path, &quic_key_path) {
        Ok(identity) => identity,
        Err(err) => {
            eprintln!("fatal: {err}");
            return;
        }
    };

    let trusted_cert_paths = env_list("VEIL_VPS_QUIC_TRUSTED_CERTS");
    let mut trusted = load_trusted_certs(&trusted_cert_paths);
    if trusted.is_empty() {
        trusted.push(identity.cert_der.clone());
    }

    let mut state = load_state_or_default(&state_path).unwrap_or_default();
    let core_tags = parse_core_tags(&core_tags);
    if !core_tags.is_empty() {
        let before = state.subscriptions.len();
        for tag in core_tags {
            state.subscriptions.insert(tag);
        }
        let added = state.subscriptions.len().saturating_sub(before);
        eprintln!("auto-subscribed to {added} core tags");
    }

    let mut cfg = NodeRuntimeConfig::edge_forwarder_hot_cache_defaults();
    cfg.max_cache_shards = max_cache_shards;
    cfg.bucket_jitter_extra_levels = bucket_jitter;
    cfg.required_signed_namespaces = required_signed;
    cfg.adaptive_lane_scoring = AdaptiveLaneScoringConfig {
        enabled: adaptive_scoring,
        ..AdaptiveLaneScoringConfig::default()
    };

    let quic_bind_addr = match quic_bind.parse() {
        Ok(addr) => addr,
        Err(err) => {
            eprintln!("fatal: invalid VEIL_VPS_QUIC_BIND: {err}");
            return;
        }
    };

    let fast_adapter_raw = match QuicAdapter::connect(QuicAdapterConfig {
        bind_addr: quic_bind_addr,
        server_name: "veil-node".to_string(),
        identity,
        trusted_peer_certs_der: trusted,
        connect_timeout: Duration::from_secs(3),
        send_timeout: Duration::from_secs(3),
        outbound_queue_capacity: 2048,
        inbound_queue_capacity: 4096,
        max_recv_bytes: 128 * 1024,
        max_payload_hint: Some(64 * 1024),
    }) {
        Ok(adapter) => adapter,
        Err(err) => {
            eprintln!("fatal: quic adapter failed to start: {err}");
            return;
        }
    };

    let ws_adapter = ws_url.map(|url| {
        WebSocketAdapter::connect(WebSocketAdapterConfig {
            url,
            peer_id: ws_peer_id.clone(),
            reconnect: true,
            reconnect_initial: Duration::from_millis(250),
            reconnect_max: Duration::from_secs(10),
            outbound_queue_capacity: 1024,
            inbound_queue_capacity: 4096,
            max_payload_hint: Some(64 * 1024),
        })
        .expect("websocket adapter should start")
    });

    let tor_adapter = tor_socks_addr.map(|addr| {
        TorSocksAdapter::connect(TorSocksAdapterConfig {
            socks_proxy_addr: addr,
            connect_timeout: Duration::from_secs(8),
            send_timeout: Duration::from_secs(8),
            outbound_queue_capacity: 1024,
            max_payload_hint: Some(64 * 1024),
        })
        .expect("tor adapter should start")
    });

    let fallback_adapter = CombinedFallbackAdapter::new(ws_adapter, tor_adapter);
    let fallback_peers = parse_fallback_peers(ws_peer, tor_peers);

    let discovered_fast = Arc::new(Mutex::new(HashSet::new()));
    let discovered_fallback = Arc::new(Mutex::new(HashSet::new()));

    let fast_adapter = RecordingAdapter::new(fast_adapter_raw, Arc::clone(&discovered_fast));
    let fallback_adapter = RecordingAdapter::new(fallback_adapter, Arc::clone(&discovered_fallback));

    let discovered_seed = load_peer_list(&peer_list_path);
    {
        let mut guard = discovered_fast.lock().unwrap_or_else(|e| e.into_inner());
        for peer in discovered_seed
            .iter()
            .filter(|p| !p.starts_with("ws:") && !p.starts_with("tor:"))
        {
            guard.insert(peer.to_string());
        }
    }
    let fallback_seed = parse_fallback_peer_strings(&discovered_seed);
    {
        let mut guard = discovered_fallback
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        for peer in fallback_seed {
            guard.insert(peer);
        }
    }

    let mut runtime = NodeRuntime::new(
        state,
        fast_adapter,
        fallback_adapter,
        cfg,
        node_key,
        XChaCha20Poly1305Cipher,
        Ed25519Verifier,
    );

    let tick_interval = Duration::from_millis(tick_ms);
    let snapshot_interval = Duration::from_secs(snapshot_secs);
    let mut last_snapshot = Instant::now();

    let mut last_health_log = Instant::now();
    let health_log_interval = Duration::from_secs(30);

    let metrics = Arc::new(MetricsState::default());
    start_health_server(health_port, Arc::clone(&metrics));

    let mut now_step = 0_u64;
    loop {
        let metrics_ref = Arc::clone(&metrics);
        let discovered_fast_snapshot = runtime.fast_adapter.snapshot_seen();
        let discovered_fallback_snapshot = runtime.fallback_adapter.snapshot_seen();
        let fast_peer_list = merge_peers(&fast_peers, &discovered_fast_snapshot, max_dynamic_peers);
        let fallback_peer_list = merge_peers(
            &fallback_peers,
            &discovered_fallback_snapshot,
            max_dynamic_peers,
        );
        let _ = runtime.tick_with_callbacks(
            now_step,
            &fast_peer_list,
            &fallback_peer_list,
            NodeRuntimeCallbacks {
                on_delivered: Some(&mut |_root, _payload| {
                    metrics_ref.delivered.fetch_add(1, Ordering::Relaxed);
                }),
                on_send_failure: Some(&mut |count| {
                    metrics_ref
                        .send_failures
                        .fetch_add(count as u64, Ordering::Relaxed);
                }),
                on_ack_cleared: Some(&mut |count| {
                    metrics_ref
                        .ack_clears
                        .fetch_add(count as u64, Ordering::Relaxed);
                }),
                ..NodeRuntimeCallbacks::default()
            },
        );
        now_step = now_step.saturating_add(1);
        metrics.ticks.fetch_add(1, Ordering::Relaxed);

        if last_snapshot.elapsed() >= snapshot_interval {
            if let Err(err) = save_state_to_path(&state_path, &runtime.state) {
                eprintln!("snapshot failed: {err}");
            }
            let mut fast_snapshot = runtime.fast_adapter.snapshot_seen();
            fast_snapshot.sort();
            let mut fallback_snapshot =
                encode_fallback_peers(&runtime.fallback_adapter.snapshot_seen());
            fallback_snapshot.sort();
            let mut merged = fast_snapshot;
            merged.extend(fallback_snapshot);
            merged.sort();
            merged.dedup();
            save_peer_list(&peer_list_path, &merged);
            last_snapshot = Instant::now();
        }

        if last_health_log.elapsed() >= health_log_interval {
            let health = runtime.transport_health();
            metrics
                .last_fast_outbound_ok
                .store(health.fast_lane.outbound_send_ok, Ordering::Relaxed);
            metrics
                .last_fast_outbound_err
                .store(health.fast_lane.outbound_send_err, Ordering::Relaxed);
            metrics
                .last_fallback_outbound_ok
                .store(health.fallback_lane.outbound_send_ok, Ordering::Relaxed);
            metrics
                .last_fallback_outbound_err
                .store(health.fallback_lane.outbound_send_err, Ordering::Relaxed);
            metrics
                .last_fast_inbound
                .store(health.fast_lane.inbound_received, Ordering::Relaxed);
            metrics
                .last_fallback_inbound
                .store(health.fallback_lane.inbound_received, Ordering::Relaxed);
            eprintln!(
                "fast_lane: {:?}, fallback_lane: {:?}",
                health.fast_lane, health.fallback_lane
            );
            last_health_log = Instant::now();
        }

        thread::sleep(tick_interval);
    }
}
