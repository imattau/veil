use std::collections::{HashMap, HashSet};
use std::fs;
use std::hash::Hash;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

mod nostr_bridge;
mod settings_db;

use bech32::Hrp;
use nostr_bridge::{start_nostr_bridge, NostrBridgeConfig};
use rand::RngCore;
use rusqlite::{params, Connection};
use serde::Deserialize;
use serde_json::json;
use settings_db::SettingsStore;
use signal_hook::consts::{SIGINT, SIGTERM};
use signal_hook::flag;
use veil_core::hash::blake3_32;
use veil_core::tags::derive_channel_feed_tag;
use veil_core::{Epoch, Namespace};
use veil_crypto::aead::XChaCha20Poly1305Cipher;
use veil_crypto::signing::{NostrSigner, NostrVerifier, Signer};
use veil_node::batch::FeedBatcher;
use veil_node::config::{
    AdaptiveLaneScoringConfig, BloomExchangeConfig, NodeRuntimeConfig,
    ProbabilisticForwardingConfig,
};
use veil_node::persistence::{load_state_or_default, save_state_to_path};
use veil_node::publish::{publish_queue_tick_multi_lane, PublishQueueTickParams};
use veil_node::service::{NodeRuntime, NodeRuntimeCallbacks};
use veil_transport::adapter::{TransportAdapter, TransportHealthSnapshot};
#[cfg(feature = "ble-btleplug")]
use veil_transport_ble::btleplug_backend::{BtleplugLink, BtleplugLinkConfig};
#[cfg(all(feature = "ble", not(feature = "ble-btleplug")))]
use veil_transport_ble::MockBleLink;
#[cfg(feature = "ble")]
use veil_transport_ble::{BleAdapter, BleAdapterConfig, BlePeer};
use veil_transport_quic::{QuicAdapter, QuicAdapterConfig, QuicIdentity};
use veil_transport_tor::{TorSocksAdapter, TorSocksAdapterConfig};
use veil_transport_websocket::{
    WebSocketAdapter, WebSocketAdapterConfig, WebSocketServerAdapter, WebSocketServerAdapterConfig,
};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum FallbackPeer {
    WebSocket(String),
    WebSocketServer(String),
    Tor(String),
    #[cfg(feature = "ble")]
    Ble(BlePeer),
}

impl std::fmt::Display for FallbackPeer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FallbackPeer::WebSocket(peer) => write!(f, "ws:{peer}"),
            FallbackPeer::WebSocketServer(peer) => write!(f, "wssrv:{peer}"),
            FallbackPeer::Tor(peer) => write!(f, "tor:{peer}"),
            #[cfg(feature = "ble")]
            FallbackPeer::Ble(peer) => write!(f, "ble:{}", peer.addr),
        }
    }
}

#[derive(Debug)]
enum FallbackSendError {
    WebSocket,
    WebSocketServer,
    Tor,
    MissingWebSocket,
    MissingWebSocketServer,
    MissingTor,
    #[cfg(feature = "ble")]
    Ble,
    #[cfg(feature = "ble")]
    MissingBle,
}

#[cfg(feature = "ble-btleplug")]
type BleLinkImpl = BtleplugLink;
#[cfg(all(feature = "ble", not(feature = "ble-btleplug")))]
type BleLinkImpl = MockBleLink;

struct CombinedFallbackAdapter {
    ws: Option<WebSocketAdapter>,
    ws_server: Option<WebSocketServerAdapter>,
    tor: Option<TorSocksAdapter>,
    #[cfg(feature = "ble")]
    ble: Option<BleAdapter<BleLinkImpl>>,
}

impl CombinedFallbackAdapter {
    fn new(
        ws: Option<WebSocketAdapter>,
        ws_server: Option<WebSocketServerAdapter>,
        tor: Option<TorSocksAdapter>,
        #[cfg(feature = "ble")] ble: Option<BleAdapter<BleLinkImpl>>,
    ) -> Self {
        Self {
            ws,
            ws_server,
            tor,
            #[cfg(feature = "ble")]
            ble,
        }
    }

    fn ws_mut(&mut self) -> Option<&mut WebSocketAdapter> {
        self.ws.as_mut()
    }

    fn ws_server_mut(&mut self) -> Option<&mut WebSocketServerAdapter> {
        self.ws_server.as_mut()
    }

    fn tor_mut(&mut self) -> Option<&mut TorSocksAdapter> {
        self.tor.as_mut()
    }

    #[cfg(feature = "ble")]
    fn ble_mut(&mut self) -> Option<&mut BleAdapter<BleLinkImpl>> {
        self.ble.as_mut()
    }

    fn combined_max_payload_hint(&self) -> Option<usize> {
        let ws_hint = self.ws.as_ref().and_then(|w| w.max_payload_hint());
        let ws_srv_hint = self.ws_server.as_ref().and_then(|w| w.max_payload_hint());
        let tor_hint = self.tor.as_ref().and_then(|t| t.max_payload_hint());
        let hint = match (ws_hint, ws_srv_hint, tor_hint) {
            (Some(a), Some(b), Some(c)) => Some(a.min(b).min(c)),
            (Some(a), Some(b), None) => Some(a.min(b)),
            (Some(a), None, Some(c)) => Some(a.min(c)),
            (None, Some(b), Some(c)) => Some(b.min(c)),
            (Some(a), None, None) => Some(a),
            (None, Some(b), None) => Some(b),
            (None, None, Some(c)) => Some(c),
            (None, None, None) => None,
        };
        #[cfg(feature = "ble")]
        {
            let ble_hint = self.ble.as_ref().and_then(|b| b.max_payload_hint());
            return match (hint, ble_hint) {
                (Some(a), Some(b)) => Some(a.min(b)),
                (Some(a), None) => Some(a),
                (None, Some(b)) => Some(b),
                (None, None) => None,
            };
        }
        hint
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
        if let Some(ws) = &self.ws_server {
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
        #[cfg(feature = "ble")]
        if let Some(ble) = &self.ble {
            let h = ble.health_snapshot();
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
            FallbackPeer::WebSocketServer(ws_peer) => {
                let ws = self
                    .ws_server_mut()
                    .ok_or(FallbackSendError::MissingWebSocketServer)?;
                ws.send(ws_peer, bytes)
                    .map_err(|_| FallbackSendError::WebSocketServer)
            }
            FallbackPeer::Tor(tor_peer) => {
                let tor = self.tor_mut().ok_or(FallbackSendError::MissingTor)?;
                tor.send(tor_peer, bytes)
                    .map_err(|_| FallbackSendError::Tor)
            }
            #[cfg(feature = "ble")]
            FallbackPeer::Ble(ble_peer) => {
                let ble = self.ble_mut().ok_or(FallbackSendError::MissingBle)?;
                ble.send(ble_peer, bytes)
                    .map_err(|_| FallbackSendError::Ble)
            }
        }
    }

    fn recv(&mut self) -> Option<(Self::Peer, Vec<u8>)> {
        if let Some(ws) = self.ws_mut() {
            if let Some((peer, bytes)) = ws.recv() {
                return Some((FallbackPeer::WebSocket(peer), bytes));
            }
        }
        if let Some(ws) = self.ws_server_mut() {
            if let Some((peer, bytes)) = ws.recv() {
                return Some((FallbackPeer::WebSocketServer(peer), bytes));
            }
        }
        #[cfg(feature = "ble")]
        if let Some(ble) = self.ble_mut() {
            if let Some((peer, bytes)) = ble.recv() {
                return Some((FallbackPeer::Ble(peer), bytes));
            }
        }
        None
    }

    fn max_payload_hint(&self) -> Option<usize> {
        self.combined_max_payload_hint()
    }

    fn can_send(&self) -> bool {
        let ok = self.ws.as_ref().map(|w| w.can_send()).unwrap_or(false)
            || self
                .ws_server
                .as_ref()
                .map(|w| w.can_send())
                .unwrap_or(false)
            || self.tor.as_ref().map(|t| t.can_send()).unwrap_or(false);
        #[cfg(feature = "ble")]
        {
            return ok || self.ble.as_ref().map(|b| b.can_send()).unwrap_or(false);
        }
        ok
    }

    fn can_recv(&self) -> bool {
        let ok = self.ws.as_ref().map(|w| w.can_recv()).unwrap_or(false)
            || self
                .ws_server
                .as_ref()
                .map(|w| w.can_recv())
                .unwrap_or(false);
        #[cfg(feature = "ble")]
        {
            return ok || self.ble.as_ref().map(|b| b.can_recv()).unwrap_or(false);
        }
        ok
    }

    fn health_snapshot(&self) -> TransportHealthSnapshot {
        self.combined_health_snapshot()
    }
}

#[cfg(feature = "ble")]
impl FallbackPeer {
    fn peer_ble(self) -> BlePeer {
        match self {
            FallbackPeer::Ble(p) => p,
            _ => panic!("not a ble peer"),
        }
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

fn setting_string(store: Option<&SettingsStore>, key: &str, default: &str) -> String {
    if let Some(store) = store {
        if let Some(value) = store.get(key) {
            return value;
        }
        let _ = store.set_if_absent(key, default);
    }
    default.to_string()
}

fn setting_opt_string(store: Option<&SettingsStore>, key: &str) -> Option<String> {
    store.and_then(|s| s.get(key)).and_then(|v| {
        let trimmed = v.trim().to_string();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    })
}

fn setting_u64(store: Option<&SettingsStore>, key: &str, default: u64) -> u64 {
    setting_string(store, key, &default.to_string())
        .parse::<u64>()
        .unwrap_or(default)
}

fn setting_f64(store: Option<&SettingsStore>, key: &str, default: f64) -> f64 {
    setting_string(store, key, &default.to_string())
        .parse::<f64>()
        .unwrap_or(default)
}

fn setting_usize(store: Option<&SettingsStore>, key: &str, default: usize) -> usize {
    setting_string(store, key, &default.to_string())
        .parse::<usize>()
        .unwrap_or(default)
}

fn setting_bool(store: Option<&SettingsStore>, key: &str, default: bool) -> bool {
    let value = setting_string(store, key, if default { "1" } else { "0" });
    matches!(value.as_str(), "1" | "true" | "TRUE" | "yes" | "YES")
}

fn setting_list(store: Option<&SettingsStore>, key: &str, default: &[&str]) -> Vec<String> {
    let fallback = default.join(",");
    setting_string(store, key, &fallback)
        .split(',')
        .map(|entry| entry.trim())
        .filter(|entry| !entry.is_empty())
        .map(|entry| entry.to_string())
        .collect()
}

fn current_epoch() -> Epoch {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    Epoch((now / 86_400) as u32)
}

fn open_peer_db(path: &Path) -> Option<Connection> {
    if let Err(err) = ensure_parent(path) {
        eprintln!("failed to create peer db dir: {err}");
        return None;
    }
    let conn = match Connection::open(path) {
        Ok(conn) => conn,
        Err(err) => {
            eprintln!("failed to open peer db: {err}");
            return None;
        }
    };
    let _ = conn.execute_batch(
        "PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL; PRAGMA busy_timeout=5000;",
    );
    if let Err(err) = conn.execute(
        "CREATE TABLE IF NOT EXISTS peers (peer TEXT PRIMARY KEY, last_seen_ms INTEGER NOT NULL)",
        [],
    ) {
        eprintln!("failed to init peer db: {err}");
        return None;
    }
    Some(conn)
}

fn load_peer_list(conn: &Connection, limit: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut stmt = match conn.prepare("SELECT peer FROM peers ORDER BY last_seen_ms DESC LIMIT ?1")
    {
        Ok(stmt) => stmt,
        Err(_) => return out,
    };
    let rows = stmt.query_map([limit as i64], |row| row.get::<_, String>(0));
    if let Ok(rows) = rows {
        for row in rows.flatten() {
            out.push(row);
        }
    }
    out
}

fn save_peer_list(conn: &Connection, peers: &[String]) {
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;
    for peer in peers {
        let _ = conn.execute(
            "INSERT INTO peers (peer, last_seen_ms) VALUES (?1, ?2)\n             ON CONFLICT(peer) DO UPDATE SET last_seen_ms=excluded.last_seen_ms",
            params![peer, now_ms],
        );
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
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(cert_path, fs::Permissions::from_mode(0o600));
        let _ = fs::set_permissions(key_path, fs::Permissions::from_mode(0o600));
    }
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

fn pseudo_pubkey_for_peer(peer: &str) -> [u8; 32] {
    let mut preimage = Vec::with_capacity(8 + peer.len());
    preimage.extend_from_slice(b"vps-peer");
    preimage.extend_from_slice(peer.as_bytes());
    blake3_32(&preimage)
}

fn parse_fallback_peers(
    ws_peer: Option<String>,
    tor_peers: Vec<String>,
    #[cfg(feature = "ble")] ble_peers: Vec<String>,
) -> Vec<FallbackPeer> {
    let mut peers = Vec::new();
    if let Some(ws_peer) = ws_peer {
        peers.push(FallbackPeer::WebSocket(ws_peer));
    }
    for peer in tor_peers {
        peers.push(FallbackPeer::Tor(peer));
    }
    #[cfg(feature = "ble")]
    for peer in ble_peers {
        peers.push(FallbackPeer::Ble(BlePeer::new(peer)));
    }
    peers
}

fn parse_fallback_peer_strings(values: &[String]) -> Vec<FallbackPeer> {
    values
        .iter()
        .filter_map(|value| {
            if let Some(rest) = value.strip_prefix("ws:") {
                Some(FallbackPeer::WebSocket(rest.to_string()))
            } else if let Some(rest) = value.strip_prefix("wssrv:") {
                Some(FallbackPeer::WebSocketServer(rest.to_string()))
            } else if let Some(rest) = value.strip_prefix("tor:") {
                Some(FallbackPeer::Tor(rest.to_string()))
            } else if let Some(_rest) = value.strip_prefix("ble:") {
                #[cfg(feature = "ble")]
                {
                    Some(FallbackPeer::Ble(BlePeer::new(_rest.to_string())))
                }
                #[cfg(not(feature = "ble"))]
                {
                    None
                }
            } else {
                None
            }
        })
        .collect()
}

fn fallback_peer_supported(
    peer: &FallbackPeer,
    ws_enabled: bool,
    ws_server_enabled: bool,
    tor_enabled: bool,
    #[cfg(feature = "ble")] ble_enabled: bool,
) -> bool {
    match peer {
        FallbackPeer::WebSocket(_) => ws_enabled,
        FallbackPeer::WebSocketServer(_) => ws_server_enabled,
        FallbackPeer::Tor(_) => tor_enabled,
        #[cfg(feature = "ble")]
        FallbackPeer::Ble(_) => ble_enabled,
    }
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
            if NostrSigner::from_secret(out).is_ok() {
                return Ok(out);
            }
            return Err("existing node key is not a valid Nostr secp256k1 secret".to_string());
        }
    }

    let key = loop {
        let mut candidate = [0_u8; 32];
        rand::thread_rng().fill_bytes(&mut candidate);
        if NostrSigner::from_secret(candidate).is_ok() {
            break candidate;
        }
    };
    ensure_parent(path).map_err(|e| format!("create node key dir: {e}"))?;
    fs::write(path, key).map_err(|e| format!("write node key: {e}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
    }
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
    nostr_bridge_events_total: AtomicU64,
    nostr_bridge_payload_bytes_total: AtomicU64,
    nostr_bridge_enabled: AtomicU64,
    nostr_bridge_relays_configured: AtomicU64,
}

#[derive(Debug)]
struct AdminAuthState {
    server_pubkey: [u8; 32],
    server_pubkey_hex: String,
    session_ttl_secs: u64,
    session_db_path: PathBuf,
    settings_db_path: PathBuf,
    sessions: Mutex<HashMap<String, u64>>,
}

#[derive(Debug, Deserialize)]
struct AdminLoginRequest {
    secret: String,
}

#[derive(Debug, Deserialize)]
struct AdminSettingUpsertRequest {
    key: String,
    value: String,
}

impl AdminAuthState {
    fn bootstrap_session_db(path: &Path) {
        if let Err(err) = ensure_parent(path) {
            eprintln!(
                "admin auth: failed to create session db parent {}: {err}",
                path.display()
            );
            return;
        }
        match Connection::open(path) {
            Ok(conn) => {
                if let Err(err) = conn.execute(
                    "CREATE TABLE IF NOT EXISTS admin_sessions (
                        token TEXT PRIMARY KEY,
                        expires_at INTEGER NOT NULL
                    )",
                    params![],
                ) {
                    eprintln!(
                        "admin auth: failed to initialize session table {}: {err}",
                        path.display()
                    );
                    return;
                }
                let now = now_unix_secs() as i64;
                let _ = conn.execute(
                    "DELETE FROM admin_sessions WHERE expires_at <= ?1",
                    params![now],
                );
            }
            Err(err) => {
                eprintln!(
                    "admin auth: failed to open session db {}: {err}",
                    path.display()
                );
            }
        }
    }

    fn load_sessions_from_db(path: &Path) -> HashMap<String, u64> {
        let mut out = HashMap::new();
        let Ok(conn) = Connection::open(path) else {
            return out;
        };
        let now = now_unix_secs() as i64;
        let _ = conn.execute(
            "DELETE FROM admin_sessions WHERE expires_at <= ?1",
            params![now],
        );
        let Ok(mut stmt) = conn.prepare("SELECT token, expires_at FROM admin_sessions") else {
            return out;
        };
        let rows = stmt.query_map(params![], |row| {
            let token: String = row.get(0)?;
            let expires_at: i64 = row.get(1)?;
            Ok((token, expires_at))
        });
        if let Ok(rows) = rows {
            for (token, expires_at) in rows.flatten() {
                if expires_at > 0 {
                    out.insert(token, expires_at as u64);
                }
            }
        }
        out
    }

    fn persist_session_insert(&self, token: &str, expires: u64) {
        if let Ok(conn) = Connection::open(&self.session_db_path) {
            let _ = conn.execute(
                "INSERT OR REPLACE INTO admin_sessions (token, expires_at) VALUES (?1, ?2)",
                params![token, expires as i64],
            );
        }
    }

    fn persist_session_remove(&self, token: &str) {
        if let Ok(conn) = Connection::open(&self.session_db_path) {
            let _ = conn.execute(
                "DELETE FROM admin_sessions WHERE token = ?1",
                params![token],
            );
        }
    }

    fn persist_expired_prune(&self, now: u64) {
        if let Ok(conn) = Connection::open(&self.session_db_path) {
            let _ = conn.execute(
                "DELETE FROM admin_sessions WHERE expires_at <= ?1",
                params![now as i64],
            );
        }
    }

    fn add_session(&self, token: String, expires: u64) {
        self.sessions
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(token.clone(), expires);
        self.persist_session_insert(&token, expires);
    }

    fn revoke_session(&self, token: &str) -> bool {
        let removed = self
            .sessions
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(token)
            .is_some();
        self.persist_session_remove(token);
        removed
    }
}

fn now_unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn decode_hex_exact<const N: usize>(value: &str) -> Option<[u8; N]> {
    if value.len() != N * 2 {
        return None;
    }
    let mut out = [0_u8; N];
    for (idx, chunk) in value.as_bytes().chunks_exact(2).enumerate() {
        let s = std::str::from_utf8(chunk).ok()?;
        out[idx] = u8::from_str_radix(s, 16).ok()?;
    }
    Some(out)
}

fn encode_hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

fn decode_nostr_secret_input(value: &str) -> Option<[u8; 32]> {
    let trimmed = value.trim();
    if let Some(key) = decode_hex_exact::<32>(trimmed) {
        return Some(key);
    }
    let hrp = Hrp::parse("nsec").ok()?;
    let (decoded_hrp, data) = bech32::decode(trimmed).ok()?;
    if decoded_hrp != hrp {
        return None;
    }
    if data.len() != 32 {
        return None;
    }
    let mut out = [0_u8; 32];
    out.copy_from_slice(&data);
    Some(out)
}

fn read_http_request(
    stream: &mut std::net::TcpStream,
) -> Option<(String, String, HashMap<String, String>, Vec<u8>)> {
    let mut buf = Vec::new();
    let mut tmp = [0_u8; 2048];
    let mut header_end: Option<usize> = None;
    let mut expected_body_len = 0usize;
    loop {
        let read = stream.read(&mut tmp).ok()?;
        if read == 0 {
            break;
        }
        buf.extend_from_slice(&tmp[..read]);
        if header_end.is_none() {
            if let Some(idx) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                header_end = Some(idx + 4);
                let header_text = String::from_utf8_lossy(&buf[..idx + 4]);
                for line in header_text.lines().skip(1) {
                    let mut parts = line.splitn(2, ':');
                    let key = parts.next().unwrap_or("").trim().to_ascii_lowercase();
                    let value = parts.next().unwrap_or("").trim();
                    if key == "content-length" {
                        expected_body_len = value.parse::<usize>().unwrap_or(0).min(16 * 1024);
                        break;
                    }
                }
            }
        }
        if let Some(h_end) = header_end {
            let body_len = buf.len().saturating_sub(h_end);
            if body_len >= expected_body_len {
                break;
            }
        }
        if buf.len() > 64 * 1024 {
            return None;
        }
    }

    let header_end = header_end?;
    let header = String::from_utf8_lossy(&buf[..header_end]).to_string();
    let mut lines = header.lines();
    let request_line = lines.next()?.to_string();
    let mut parts = request_line.split_whitespace();
    let method = parts.next()?.to_string();
    let path = parts.next()?.to_string();
    let mut headers = HashMap::new();
    for line in lines {
        let mut fields = line.splitn(2, ':');
        let key = fields.next().unwrap_or("").trim().to_ascii_lowercase();
        let value = fields.next().unwrap_or("").trim().to_string();
        if !key.is_empty() {
            headers.insert(key, value);
        }
    }
    let body = buf[header_end..].to_vec();
    Some((method, path, headers, body))
}

fn bearer_token(headers: &HashMap<String, String>) -> Option<&str> {
    let auth = headers.get("authorization")?;
    let token = auth.strip_prefix("Bearer ")?;
    if token.is_empty() {
        return None;
    }
    Some(token)
}

fn query_param(path: &str, target_key: &str) -> Option<String> {
    let query = path.split('?').nth(1)?;
    for pair in query.split('&') {
        let mut parts = pair.splitn(2, '=');
        let key = parts.next().unwrap_or("");
        let value = parts.next().unwrap_or("");
        if key == target_key && !value.is_empty() {
            return Some(value.to_string());
        }
    }
    None
}

fn is_admin_authenticated(headers: &HashMap<String, String>, admin: &AdminAuthState) -> bool {
    let Some(token) = bearer_token(headers) else {
        return false;
    };
    let now = now_unix_secs();
    let mut sessions = admin.sessions.lock().unwrap_or_else(|e| e.into_inner());
    let expired_tokens = sessions
        .iter()
        .filter_map(|(token, expires)| {
            if *expires <= now {
                Some(token.clone())
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    sessions.retain(|_, expires| *expires > now);
    drop(sessions);
    if !expired_tokens.is_empty() {
        for token in expired_tokens {
            admin.persist_session_remove(&token);
        }
        admin.persist_expired_prune(now);
    }
    let sessions = admin.sessions.lock().unwrap_or_else(|e| e.into_inner());
    sessions.get(token).is_some_and(|expires| *expires > now)
}

fn start_health_server(
    bind_addr: String,
    port: u16,
    metrics: Arc<MetricsState>,
    peer_snapshot: Arc<Mutex<Vec<String>>>,
    admin_auth: Arc<AdminAuthState>,
) {
    if port == 0 {
        return;
    }
    thread::spawn(move || {
        let listener = match TcpListener::bind((bind_addr.as_str(), port)) {
            Ok(listener) => listener,
            Err(err) => {
                eprintln!("health server bind failed on {bind_addr}:{port}: {err}");
                return;
            }
        };
        for mut stream in listener.incoming().flatten() {
            let Some((method, path, headers, body_bytes)) = read_http_request(&mut stream) else {
                continue;
            };
            let path_only = path.split('?').next().unwrap_or(path.as_str());
            let is_health = method == "GET" && (path_only == "/health" || path_only == "/healthz");
            let is_metrics = method == "GET" && path_only == "/metrics";
            let is_peers = method == "GET" && path_only == "/peers";
            let is_admin_login = method == "POST" && path_only == "/admin-api/login";
            let is_admin_logout = method == "POST" && path_only == "/admin-api/logout";
            let is_admin_status = method == "GET" && path_only == "/admin-api/status";
            let is_admin_metrics = method == "GET" && path_only == "/admin-api/metrics";
            let is_admin_peers = method == "GET" && path_only == "/admin-api/peers";
            let is_admin_settings_get = method == "GET" && path_only == "/admin-api/settings";
            let is_admin_settings_set = method == "POST" && path_only == "/admin-api/settings";
            let is_admin_settings_delete = method == "DELETE" && path_only == "/admin-api/settings";

            let (status, body, content_type) = if is_health {
                ("200 OK", "ok".to_string(), "text/plain")
            } else if is_metrics
                || (is_admin_metrics && is_admin_authenticated(&headers, &admin_auth))
            {
                let body = format!(
                    "veil_ticks_total {}\nveil_delivered_total {}\nveil_send_failures_total {}\nveil_ack_clears_total {}\nveil_fast_outbound_ok {}\nveil_fast_outbound_err {}\nveil_fallback_outbound_ok {}\nveil_fallback_outbound_err {}\nveil_fast_inbound {}\nveil_fallback_inbound {}\nveil_nostr_bridge_events_total {}\nveil_nostr_bridge_payload_bytes_total {}\nveil_nostr_bridge_enabled {}\nveil_nostr_bridge_relays_configured {}\n",
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
                    metrics.nostr_bridge_events_total.load(Ordering::Relaxed),
                    metrics
                        .nostr_bridge_payload_bytes_total
                        .load(Ordering::Relaxed),
                    metrics.nostr_bridge_enabled.load(Ordering::Relaxed),
                    metrics
                        .nostr_bridge_relays_configured
                        .load(Ordering::Relaxed),
                );
                ("200 OK", body, "text/plain")
            } else if is_peers || (is_admin_peers && is_admin_authenticated(&headers, &admin_auth))
            {
                let mut limit = 200usize;
                let mut prefix: Option<String> = None;
                if let Some(query) = path.split('?').nth(1) {
                    for pair in query.split('&') {
                        let mut parts = pair.splitn(2, '=');
                        let key = parts.next().unwrap_or("");
                        let value = parts.next().unwrap_or("");
                        if key == "limit" {
                            if let Ok(parsed) = value.parse::<usize>() {
                                limit = parsed.min(1000);
                            }
                        } else if key == "prefix" && !value.is_empty() {
                            prefix = Some(value.to_string());
                        }
                    }
                }
                let peers = peer_snapshot.lock().unwrap_or_else(|e| e.into_inner());
                let iter = peers
                    .iter()
                    .filter(|peer| prefix.as_ref().map(|p| peer.starts_with(p)).unwrap_or(true));
                let body = iter.take(limit).cloned().collect::<Vec<_>>().join("\n");
                ("200 OK", body, "text/plain")
            } else if is_admin_login {
                let parsed = serde_json::from_slice::<AdminLoginRequest>(&body_bytes).ok();
                if let Some(payload) = parsed {
                    if let Some(secret) = decode_nostr_secret_input(&payload.secret) {
                        if let Ok(signer) = NostrSigner::from_secret(secret) {
                            if signer.public_key() == admin_auth.server_pubkey {
                                let mut raw = [0_u8; 32];
                                rand::thread_rng().fill_bytes(&mut raw);
                                let token = encode_hex(&raw);
                                let expires = now_unix_secs() + admin_auth.session_ttl_secs;
                                admin_auth.add_session(token.clone(), expires);
                                (
                                    "200 OK",
                                    json!({
                                        "ok": true,
                                        "token": token,
                                        "server_pubkey": admin_auth.server_pubkey_hex,
                                        "expires_at": expires
                                    })
                                    .to_string(),
                                    "application/json",
                                )
                            } else {
                                (
                                    "401 Unauthorized",
                                    json!({"ok": false, "error": "wrong identity key"}).to_string(),
                                    "application/json",
                                )
                            }
                        } else {
                            (
                                "400 Bad Request",
                                json!({"ok": false, "error": "invalid nostr secret"}).to_string(),
                                "application/json",
                            )
                        }
                    } else {
                        (
                            "400 Bad Request",
                            json!({"ok": false, "error": "secret must be hex or nsec"}).to_string(),
                            "application/json",
                        )
                    }
                } else {
                    (
                        "400 Bad Request",
                        json!({"ok": false, "error": "invalid JSON payload"}).to_string(),
                        "application/json",
                    )
                }
            } else if is_admin_logout {
                if let Some(token) = bearer_token(&headers) {
                    let _ = admin_auth.revoke_session(token);
                    (
                        "200 OK",
                        json!({"ok": true, "logged_out": true}).to_string(),
                        "application/json",
                    )
                } else {
                    (
                        "401 Unauthorized",
                        json!({"ok": false, "error": "admin auth required"}).to_string(),
                        "application/json",
                    )
                }
            } else if is_admin_status {
                let is_auth = is_admin_authenticated(&headers, &admin_auth);
                (
                    "200 OK",
                    json!({
                        "ok": is_auth,
                        "server_pubkey": admin_auth.server_pubkey_hex
                    })
                    .to_string(),
                    "application/json",
                )
            } else if is_admin_settings_get && is_admin_authenticated(&headers, &admin_auth) {
                match SettingsStore::open(&admin_auth.settings_db_path) {
                    Ok(store) => {
                        if let Some(key) = query_param(&path, "key") {
                            match store.get(&key) {
                                Some(value) => (
                                    "200 OK",
                                    json!({"ok": true, "key": key, "value": value}).to_string(),
                                    "application/json",
                                ),
                                None => (
                                    "404 Not Found",
                                    json!({"ok": false, "error": "setting not found"}).to_string(),
                                    "application/json",
                                ),
                            }
                        } else {
                            match store.list() {
                                Ok(items) => (
                                    "200 OK",
                                    json!({"ok": true, "items": items}).to_string(),
                                    "application/json",
                                ),
                                Err(err) => (
                                    "500 Internal Server Error",
                                    json!({"ok": false, "error": err}).to_string(),
                                    "application/json",
                                ),
                            }
                        }
                    }
                    Err(err) => (
                        "500 Internal Server Error",
                        json!({"ok": false, "error": err}).to_string(),
                        "application/json",
                    ),
                }
            } else if is_admin_settings_set && is_admin_authenticated(&headers, &admin_auth) {
                let parsed = serde_json::from_slice::<AdminSettingUpsertRequest>(&body_bytes).ok();
                if let Some(payload) = parsed {
                    let key = payload.key.trim().to_string();
                    if key.is_empty() {
                        (
                            "400 Bad Request",
                            json!({"ok": false, "error": "key is required"}).to_string(),
                            "application/json",
                        )
                    } else {
                        match SettingsStore::open(&admin_auth.settings_db_path)
                            .and_then(|store| store.set(&key, payload.value.trim()))
                        {
                            Ok(()) => (
                                "200 OK",
                                json!({"ok": true, "key": key}).to_string(),
                                "application/json",
                            ),
                            Err(err) => (
                                "500 Internal Server Error",
                                json!({"ok": false, "error": err}).to_string(),
                                "application/json",
                            ),
                        }
                    }
                } else {
                    (
                        "400 Bad Request",
                        json!({"ok": false, "error": "invalid JSON payload"}).to_string(),
                        "application/json",
                    )
                }
            } else if is_admin_settings_delete && is_admin_authenticated(&headers, &admin_auth) {
                if let Some(key) = query_param(&path, "key") {
                    match SettingsStore::open(&admin_auth.settings_db_path)
                        .and_then(|store| store.delete(&key))
                    {
                        Ok(true) => (
                            "200 OK",
                            json!({"ok": true, "deleted": true, "key": key}).to_string(),
                            "application/json",
                        ),
                        Ok(false) => (
                            "404 Not Found",
                            json!({"ok": false, "error": "setting not found"}).to_string(),
                            "application/json",
                        ),
                        Err(err) => (
                            "500 Internal Server Error",
                            json!({"ok": false, "error": err}).to_string(),
                            "application/json",
                        ),
                    }
                } else {
                    (
                        "400 Bad Request",
                        json!({"ok": false, "error": "key query parameter is required"})
                            .to_string(),
                        "application/json",
                    )
                }
            } else if is_admin_metrics || is_admin_peers || is_admin_logout {
                (
                    "401 Unauthorized",
                    json!({"ok": false, "error": "admin auth required"}).to_string(),
                    "application/json",
                )
            } else if is_admin_settings_get || is_admin_settings_set || is_admin_settings_delete {
                (
                    "401 Unauthorized",
                    json!({"ok": false, "error": "admin auth required"}).to_string(),
                    "application/json",
                )
            } else {
                ("404 Not Found", "not found".to_string(), "text/plain")
            };
            let resp = format!(
                "HTTP/1.1 {status}\r\nAccess-Control-Allow-Origin: *\r\nContent-Length: {}\r\nContent-Type: {content_type}\r\n\r\n{body}",
                body.len(),
            );
            let _ = stream.write_all(resp.as_bytes());
        }
    });
}

#[cfg(test)]
mod tests {
    use super::{encode_fallback_peers, parse_fallback_peer_strings, FallbackPeer};

    #[test]
    fn parse_fallback_peer_strings_supports_websocket_server_prefix() {
        let parsed = parse_fallback_peer_strings(&[
            "wssrv:127.0.0.1:8080".to_string(),
            "ws:relay-a".to_string(),
            "tor:peer.onion:5000".to_string(),
        ]);
        assert!(parsed.contains(&FallbackPeer::WebSocketServer("127.0.0.1:8080".to_string())));
        assert!(parsed.contains(&FallbackPeer::WebSocket("relay-a".to_string())));
        assert!(parsed.contains(&FallbackPeer::Tor("peer.onion:5000".to_string())));
    }

    #[test]
    fn encode_and_parse_roundtrip_keeps_websocket_server_peers() {
        let peers = vec![
            FallbackPeer::WebSocket("relay-a".to_string()),
            FallbackPeer::WebSocketServer("192.168.1.10:8080".to_string()),
            FallbackPeer::Tor("peer.onion:5000".to_string()),
        ];
        let encoded = encode_fallback_peers(&peers);
        let decoded = parse_fallback_peer_strings(&encoded);
        assert_eq!(decoded, peers);
    }
}

fn main() {
    let mut args = std::env::args().collect::<Vec<_>>();
    if args.len() >= 2 && args[1] == "settings" {
        let mut db_path = PathBuf::from("data/settings.db");
        if args.len() >= 4 && args[2] == "--db" {
            db_path = PathBuf::from(args[3].clone());
            args.drain(2..4);
        }
        let store = match SettingsStore::open(&db_path) {
            Ok(store) => store,
            Err(err) => {
                eprintln!("settings db open failed: {err}");
                std::process::exit(1);
            }
        };
        if args.len() < 3 {
            eprintln!(
                "usage: veil-vps-node settings [--db <path>] <list|get|set|delete> [key] [value]"
            );
            std::process::exit(2);
        }
        match args[2].as_str() {
            "list" => match store.list() {
                Ok(items) => {
                    for (k, v) in items {
                        println!("{k}={v}");
                    }
                    return;
                }
                Err(err) => {
                    eprintln!("{err}");
                    std::process::exit(1);
                }
            },
            "get" => {
                if args.len() < 4 {
                    eprintln!("usage: veil-vps-node settings get <key>");
                    std::process::exit(2);
                }
                let key = &args[3];
                if let Some(v) = store.get(key) {
                    println!("{v}");
                    return;
                }
                std::process::exit(3);
            }
            "set" => {
                if args.len() < 5 {
                    eprintln!("usage: veil-vps-node settings set <key> <value>");
                    std::process::exit(2);
                }
                let key = &args[3];
                let value = &args[4];
                if let Err(err) = store.set(key, value) {
                    eprintln!("{err}");
                    std::process::exit(1);
                }
                println!("ok");
                return;
            }
            "delete" => {
                if args.len() < 4 {
                    eprintln!("usage: veil-vps-node settings delete <key>");
                    std::process::exit(2);
                }
                let key = &args[3];
                match store.delete(key) {
                    Ok(true) => {
                        println!("deleted");
                        return;
                    }
                    Ok(false) => std::process::exit(3),
                    Err(err) => {
                        eprintln!("{err}");
                        std::process::exit(1);
                    }
                }
            }
            _ => {
                eprintln!(
                    "usage: veil-vps-node settings [--db <path>] <list|get|set|delete> [key] [value]"
                );
                std::process::exit(2);
            }
        }
    }

    let settings_db_path = PathBuf::from("data/settings.db");
    let settings_store = match SettingsStore::open(&settings_db_path) {
        Ok(store) => Some(store),
        Err(err) => {
            eprintln!("settings db disabled: {err}");
            None
        }
    };
    if let Some(store) = settings_store.as_ref() {
        let import_path = PathBuf::from("/opt/veil-vps-node/veil-vps-node.env");
        if store.is_empty() && import_path.exists() {
            if let Ok(imported) = store.import_env_file(&import_path) {
                eprintln!(
                    "settings db initialized from {} ({} entries)",
                    import_path.display(),
                    imported
                );
            }
        }
    }
    let settings = settings_store.as_ref();
    let raw_alpn = setting_string(
        settings,
        "VEIL_VPS_QUIC_ALPN",
        "veil-quic/1,veil/1,veil-node,veil,h3,hq-29",
    );
    if !raw_alpn.trim().is_empty() {
        std::env::set_var("VEIL_QUIC_ALPN", raw_alpn.clone());
        eprintln!("quic: using VEIL_VPS_QUIC_ALPN from settings db: {raw_alpn}");
    }

    let state_path = PathBuf::from(setting_string(
        settings,
        "VEIL_VPS_STATE_PATH",
        "data/veil-vps-node-state.cbor",
    ));
    let node_key_path = PathBuf::from(setting_string(
        settings,
        "VEIL_VPS_NODE_KEY_PATH",
        "data/node_identity.key",
    ));
    let quic_cert_path = PathBuf::from(setting_string(
        settings,
        "VEIL_VPS_QUIC_CERT_PATH",
        "data/quic_cert.der",
    ));
    let quic_key_path = PathBuf::from(setting_string(
        settings,
        "VEIL_VPS_QUIC_KEY_PATH",
        "data/quic_key.der",
    ));
    let snapshot_secs = setting_u64(settings, "VEIL_VPS_SNAPSHOT_SECS", 60);
    let tick_ms = setting_u64(settings, "VEIL_VPS_TICK_MS", 50);
    let health_bind = setting_string(settings, "VEIL_VPS_HEALTH_BIND", "127.0.0.1");
    let health_port = setting_u64(settings, "VEIL_VPS_HEALTH_PORT", 9090) as u16;
    let admin_session_db_path = PathBuf::from(setting_string(
        settings,
        "VEIL_VPS_ADMIN_SESSION_DB_PATH",
        "data/admin-sessions.db",
    ));
    let fast_peers = setting_list(settings, "VEIL_VPS_FAST_PEERS", &[]);
    let core_tags = setting_list(settings, "VEIL_VPS_CORE_TAGS", &[]);
    let tor_peers = setting_list(settings, "VEIL_VPS_TOR_PEERS", &[]);
    #[cfg(feature = "ble")]
    let ble_enabled = setting_bool(settings, "VEIL_VPS_BLE_ENABLE", false);
    #[cfg(feature = "ble")]
    let ble_peers = if ble_enabled {
        setting_list(settings, "VEIL_VPS_BLE_PEERS", &[])
    } else {
        Vec::new()
    };
    #[cfg(feature = "ble")]
    let ble_allowlist = setting_list(settings, "VEIL_VPS_BLE_ALLOWLIST", &[]);
    #[cfg(feature = "ble")]
    let ble_mtu = setting_usize(settings, "VEIL_VPS_BLE_MTU", 180);
    let peer_db_path = PathBuf::from(setting_string(
        settings,
        "VEIL_VPS_PEER_DB_PATH",
        "data/peers.db",
    ));
    let max_dynamic_peers = setting_usize(settings, "VEIL_VPS_MAX_DYNAMIC_PEERS", 512);

    let quic_bind = setting_string(settings, "VEIL_VPS_QUIC_BIND", "0.0.0.0:5000");
    let ws_url = setting_opt_string(settings, "VEIL_VPS_WS_URL");
    let ws_listen = setting_opt_string(settings, "VEIL_VPS_WS_LISTEN");
    let ws_peer = setting_opt_string(settings, "VEIL_VPS_WS_PEER");
    let ws_peer_id = ws_peer.clone().unwrap_or_else(|| "ws-peer".to_string());
    let tor_socks_addr = setting_opt_string(settings, "VEIL_VPS_TOR_SOCKS_ADDR");

    let adaptive_scoring = setting_bool(settings, "VEIL_VPS_ADAPTIVE_LANE_SCORING", true);
    let probabilistic_forwarding =
        setting_bool(settings, "VEIL_VPS_PROBABILISTIC_FORWARDING", true);
    let forwarding_min_probability =
        setting_f64(settings, "VEIL_VPS_FORWARD_MIN_PROBABILITY", 0.10);
    let forwarding_replica_divisor = setting_u64(settings, "VEIL_VPS_FORWARD_REPLICA_DIVISOR", 8);
    let bloom_exchange = setting_bool(settings, "VEIL_VPS_BLOOM_EXCHANGE", true);
    let bloom_interval_steps = setting_u64(settings, "VEIL_VPS_BLOOM_INTERVAL_STEPS", 128);
    let bloom_false_positive_rate =
        setting_f64(settings, "VEIL_VPS_BLOOM_FALSE_POSITIVE_RATE", 0.05);
    let max_cache_shards = setting_usize(settings, "VEIL_VPS_MAX_CACHE_SHARDS", 200_000);
    let bucket_jitter = setting_usize(settings, "VEIL_VPS_BUCKET_JITTER", 0);
    let open_relay = setting_bool(settings, "VEIL_VPS_OPEN_RELAY", false);
    let blocked_peers = setting_list(settings, "VEIL_VPS_BLOCKED_PEERS", &[]);
    let nostr_bridge_enabled = setting_bool(settings, "VEIL_VPS_NOSTR_BRIDGE_ENABLE", false);
    let nostr_bridge_relays = setting_list(settings, "VEIL_VPS_NOSTR_RELAYS", &[]);
    let nostr_bridge_channel =
        setting_string(settings, "VEIL_VPS_NOSTR_CHANNEL_ID", "nostr-bridge");
    let nostr_bridge_namespace = setting_u64(settings, "VEIL_VPS_NOSTR_NAMESPACE", 32) as u16;
    let nostr_bridge_since_secs = setting_u64(settings, "VEIL_VPS_NOSTR_SINCE_SECS", 3600);
    let nostr_bridge_state_path = PathBuf::from(setting_string(
        settings,
        "VEIL_VPS_NOSTR_BRIDGE_STATE_PATH",
        "data/nostr-bridge-state.json",
    ));
    let nostr_bridge_max_seen = setting_usize(settings, "VEIL_VPS_NOSTR_MAX_SEEN_IDS", 20_000);
    let nostr_bridge_persist_every =
        setting_usize(settings, "VEIL_VPS_NOSTR_PERSIST_EVERY_UPDATES", 32);
    let required_signed = parse_required_signed_namespaces(&setting_list(
        settings,
        "VEIL_VPS_REQUIRED_SIGNED_NAMESPACES",
        &[],
    ));

    let node_key = match load_or_create_node_key(&node_key_path) {
        Ok(key) => key,
        Err(err) => {
            eprintln!("fatal: {err}");
            return;
        }
    };
    let node_signer = NostrSigner::from_secret(node_key).expect("node key validated");
    let node_pubkey = node_signer.public_key();
    let node_pubkey_hex = node_pubkey
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    eprintln!("node identity (nostr x-only pubkey): {node_pubkey_hex}");

    let identity = match load_or_create_identity(&quic_cert_path, &quic_key_path) {
        Ok(identity) => identity,
        Err(err) => {
            eprintln!("fatal: {err}");
            return;
        }
    };

    let trusted_cert_paths = setting_list(settings, "VEIL_VPS_QUIC_TRUSTED_CERTS", &[]);
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
    cfg.probabilistic_forwarding = ProbabilisticForwardingConfig {
        enabled: probabilistic_forwarding,
        min_probability: forwarding_min_probability.clamp(0.0, 1.0),
        replica_divisor: forwarding_replica_divisor.max(1),
    };
    cfg.bloom_exchange = BloomExchangeConfig {
        enabled: bloom_exchange,
        interval_steps: bloom_interval_steps.max(1),
        false_positive_rate: bloom_false_positive_rate.clamp(0.001, 0.5),
    };
    if open_relay {
        cfg.accept_all_tags = true;
        cfg.probabilistic_forwarding.enabled = false;
        let mut wot_cfg = cfg.wot_policy.config;
        wot_cfg.trusted_forward_quota = 1.0;
        wot_cfg.known_forward_quota = 1.0;
        wot_cfg.unknown_forward_quota = 1.0;
        wot_cfg.muted_forward_quota = 1.0;
        wot_cfg.blocked_forward_quota = 0.0;
        cfg.wot_policy.update_config(wot_cfg);
        eprintln!("open relay mode enabled: accepting all tags and full non-blocked forwarding");
    }
    for peer in blocked_peers {
        let pseudo = pseudo_pubkey_for_peer(&peer);
        cfg.bind_peer_publisher(peer.clone(), pseudo);
        cfg.wot_policy.block(pseudo);
    }

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

    let ws_server_adapter = ws_listen.map(|addr| {
        let adapter = WebSocketServerAdapter::listen(WebSocketServerAdapterConfig::new(&addr))
            .expect("websocket server should start");
        eprintln!("websocket server listening on {addr}");
        adapter
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

    #[cfg(feature = "ble")]
    let ble_adapter = if ble_enabled {
        #[cfg(feature = "ble-btleplug")]
        let link = match BtleplugLink::spawn(BtleplugLinkConfig {
            allowlist: ble_allowlist,
            ..BtleplugLinkConfig::default()
        }) {
            Ok(link) => link,
            Err(err) => {
                eprintln!("ble adapter failed to start: {err:?}");
                return;
            }
        };
        #[cfg(all(feature = "ble", not(feature = "ble-btleplug")))]
        let link = MockBleLink::with_mtu(ble_mtu);

        Some(BleAdapter::new(
            link,
            BleAdapterConfig {
                mtu: ble_mtu,
                max_payload_hint: Some(16 * 1024),
                drop_outbound: false,
            },
        ))
    } else {
        None
    };

    let fallback_adapter = CombinedFallbackAdapter::new(
        ws_adapter,
        ws_server_adapter,
        tor_adapter,
        #[cfg(feature = "ble")]
        ble_adapter,
    );
    let ws_enabled = fallback_adapter.ws.is_some();
    let ws_server_enabled = fallback_adapter.ws_server.is_some();
    let tor_enabled = fallback_adapter.tor.is_some();
    #[cfg(feature = "ble")]
    let ble_enabled_runtime = fallback_adapter.ble.is_some();
    let fallback_peers = parse_fallback_peers(
        ws_peer,
        tor_peers,
        #[cfg(feature = "ble")]
        ble_peers,
    );

    let discovered_fast = Arc::new(Mutex::new(HashSet::new()));
    let discovered_fallback = Arc::new(Mutex::new(HashSet::new()));

    let fast_adapter = RecordingAdapter::new(fast_adapter_raw, Arc::clone(&discovered_fast));
    let fallback_adapter =
        RecordingAdapter::new(fallback_adapter, Arc::clone(&discovered_fallback));

    let peer_db = open_peer_db(&peer_db_path);
    let discovered_seed = peer_db
        .as_ref()
        .map(|conn| load_peer_list(conn, max_dynamic_peers))
        .unwrap_or_default();
    {
        let mut guard = discovered_fast.lock().unwrap_or_else(|e| e.into_inner());
        for peer in discovered_seed.iter().filter(|p| {
            !p.starts_with("ws:")
                && !p.starts_with("wssrv:")
                && !p.starts_with("tor:")
                && !p.starts_with("ble:")
        }) {
            guard.insert(peer.to_string());
        }
    }
    let fallback_seed = parse_fallback_peer_strings(&discovered_seed);
    {
        let mut guard = discovered_fallback
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        for peer in fallback_seed {
            if fallback_peer_supported(
                &peer,
                ws_enabled,
                ws_server_enabled,
                tor_enabled,
                #[cfg(feature = "ble")]
                ble_enabled_runtime,
            ) {
                guard.insert(peer);
            }
        }
    }

    let mut runtime = NodeRuntime::new(
        state,
        fast_adapter,
        fallback_adapter,
        cfg,
        node_key,
        XChaCha20Poly1305Cipher,
        NostrVerifier,
    );
    let mut bridge_batcher = FeedBatcher::default();
    let nostr_bridge_rx = if nostr_bridge_enabled {
        if nostr_bridge_relays.is_empty() {
            eprintln!(
                "nostr bridge enabled but VEIL_VPS_NOSTR_RELAYS is empty; bridge not started"
            );
            None
        } else {
            eprintln!(
                "nostr bridge enabled with {} relays, channel={}, namespace={}",
                nostr_bridge_relays.len(),
                nostr_bridge_channel,
                nostr_bridge_namespace
            );
            Some(start_nostr_bridge(NostrBridgeConfig {
                relays: nostr_bridge_relays.clone(),
                channel_id: nostr_bridge_channel.clone(),
                namespace: nostr_bridge_namespace,
                since_secs: nostr_bridge_since_secs,
                state_path: Some(nostr_bridge_state_path.clone()),
                max_seen_ids: nostr_bridge_max_seen,
                persist_every_updates: nostr_bridge_persist_every,
            }))
        }
    } else {
        None
    };
    let bridge_namespace = Namespace(nostr_bridge_namespace);
    let bridge_tag = derive_channel_feed_tag(&node_pubkey, bridge_namespace, &nostr_bridge_channel);

    let tick_interval = Duration::from_millis(tick_ms);
    let snapshot_interval = Duration::from_secs(snapshot_secs);
    let mut last_snapshot = Instant::now();

    let mut last_health_log = Instant::now();
    let health_log_interval = Duration::from_secs(30);

    let metrics = Arc::new(MetricsState::default());
    metrics
        .nostr_bridge_enabled
        .store(u64::from(nostr_bridge_enabled), Ordering::Relaxed);
    metrics
        .nostr_bridge_relays_configured
        .store(nostr_bridge_relays.len() as u64, Ordering::Relaxed);
    let shutdown = Arc::new(AtomicBool::new(false));
    let _ = flag::register(SIGTERM, Arc::clone(&shutdown));
    let _ = flag::register(SIGINT, Arc::clone(&shutdown));
    let peer_snapshot: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    AdminAuthState::bootstrap_session_db(&admin_session_db_path);
    let restored_sessions = AdminAuthState::load_sessions_from_db(&admin_session_db_path);
    if !restored_sessions.is_empty() {
        eprintln!(
            "admin auth: restored {} active sessions from {}",
            restored_sessions.len(),
            admin_session_db_path.display()
        );
    }
    let admin_auth = Arc::new(AdminAuthState {
        server_pubkey: node_pubkey,
        server_pubkey_hex: node_pubkey_hex.clone(),
        session_ttl_secs: 24 * 60 * 60,
        session_db_path: admin_session_db_path,
        settings_db_path: settings_db_path.clone(),
        sessions: Mutex::new(restored_sessions),
    });
    start_health_server(
        health_bind,
        health_port,
        Arc::clone(&metrics),
        Arc::clone(&peer_snapshot),
        Arc::clone(&admin_auth),
    );

    let mut now_step = 0_u64;
    loop {
        if shutdown.load(Ordering::Relaxed) {
            if let Err(err) = save_state_to_path(&state_path, &runtime.state) {
                eprintln!("snapshot failed on shutdown: {err}");
            }
            break;
        }
        let metrics_ref = Arc::clone(&metrics);
        let discovered_fast_snapshot = runtime.fast_adapter.snapshot_seen();
        let discovered_fallback_snapshot = runtime.fallback_adapter.snapshot_seen();
        let fast_peer_list = merge_peers(&fast_peers, &discovered_fast_snapshot, max_dynamic_peers);
        let fallback_peer_list = merge_peers(
            &fallback_peers,
            &discovered_fallback_snapshot,
            max_dynamic_peers,
        );

        if let Some(rx) = &nostr_bridge_rx {
            for _ in 0..64 {
                match rx.try_recv() {
                    Ok(item) => {
                        eprintln!(
                            "nostr bridge: relay={} event={} bytes={}",
                            item.source_relay,
                            item.source_event_id,
                            item.payload.len()
                        );
                        metrics
                            .nostr_bridge_events_total
                            .fetch_add(1, Ordering::Relaxed);
                        metrics
                            .nostr_bridge_payload_bytes_total
                            .fetch_add(item.payload.len() as u64, Ordering::Relaxed);
                        bridge_batcher.enqueue(item.payload);
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => break,
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => break,
                }
            }
            let _ = publish_queue_tick_multi_lane(
                &mut runtime.state,
                &mut runtime.fast_adapter,
                &mut runtime.fallback_adapter,
                &mut bridge_batcher,
                PublishQueueTickParams {
                    namespace: bridge_namespace,
                    epoch: current_epoch(),
                    tag: bridge_tag,
                    encrypt_key: &runtime.decrypt_key,
                    now_step,
                    flags: veil_codec::object::OBJECT_FLAG_SIGNED,
                    interactive_flush: false,
                    fast_peers: &fast_peer_list,
                    fallback_peers: &fallback_peer_list,
                },
                &runtime.config,
                &XChaCha20Poly1305Cipher,
                Some(&node_signer),
            );
        }

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
            if let Some(conn) = peer_db.as_ref() {
                save_peer_list(conn, &merged);
            }
            {
                let mut guard = peer_snapshot.lock().unwrap_or_else(|e| e.into_inner());
                *guard = merged;
            }
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
