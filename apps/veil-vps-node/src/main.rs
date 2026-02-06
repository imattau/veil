use std::collections::HashSet;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
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
    let fast_peers = env_list("VEIL_VPS_FAST_PEERS");
    let tor_peers = env_list("VEIL_VPS_TOR_PEERS");

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

    let state = load_state_or_default(&state_path).unwrap_or_default();

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

    let fast_adapter = match QuicAdapter::connect(QuicAdapterConfig {
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

    let mut now_step = 0_u64;
    loop {
        let _ = runtime.tick_with_callbacks(
            now_step,
            &fast_peers,
            &fallback_peers,
            NodeRuntimeCallbacks::default(),
        );
        now_step = now_step.saturating_add(1);

        if last_snapshot.elapsed() >= snapshot_interval {
            if let Err(err) = save_state_to_path(&state_path, &runtime.state) {
                eprintln!("snapshot failed: {err}");
            }
            last_snapshot = Instant::now();
        }

        if last_health_log.elapsed() >= health_log_interval {
            let health = runtime.transport_health();
            eprintln!(
                "fast_lane: {:?}, fallback_lane: {:?}",
                health.fast_lane, health.fallback_lane
            );
            last_health_log = Instant::now();
        }

        thread::sleep(tick_interval);
    }
}
