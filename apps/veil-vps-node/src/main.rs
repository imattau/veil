use std::collections::{HashMap, HashSet};
use std::fs;
use std::hash::Hash;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use clap::{Parser, Subcommand};
use tracing::{error, info, warn};

mod config;
mod http_server;
mod nostr_bridge;
mod settings_db;

use bech32::{Bech32, Hrp};
use nostr_bridge::{start_nostr_bridge, NostrBridgeConfig};
use rand::RngCore;
use rusqlite::{params, Connection};
use serde::Deserialize;
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

use crate::config::VpsConfig;

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

fn current_epoch() -> Epoch {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    Epoch((now / 86_400) as u32)
}

fn open_peer_db(path: &Path) -> Option<Connection> {
    if let Err(err) = ensure_parent(path) {
        error!("failed to create peer db dir: {err}");
        return None;
    }
    let conn = match Connection::open(path) {
        Ok(conn) => conn,
        Err(err) => {
            error!("failed to open peer db: {err}");
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

fn parse_core_tags(values: &[String]) -> Vec<[u8; 32]> {
    values
        .iter()
        .filter_map(|value| {
            let bytes = hex::decode(value).ok()?;
            <[u8; 32]>::try_from(bytes.as_slice()).ok()
        })
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
    let mut seen = HashSet::with_capacity(configured.len() + discovered.len());
    let mut out = Vec::new();
    for peer in configured {
        if seen.insert(peer) {
            out.push(peer.clone());
        }
    }
    for peer in discovered {
        if out.len() >= max_total {
            break;
        }
        if seen.insert(peer) {
            out.push(peer.clone());
        }
    }
    out
}

fn load_or_create_node_key(path: &Path) -> Result<[u8; 32], String> {
    if let Ok(env_key) = std::env::var("VEIL_VPS_NODE_KEY") {
        if let Some(key) = decode_nostr_secret_input(&env_key) {
            info!("using node key from VEIL_VPS_NODE_KEY environment variable");
            return Ok(key);
        }
    }

    if path.exists() {
        let bytes = fs::read(path).map_err(|e| format!("read node key: {e}"))?;
        if bytes.len() == 32 {
            let mut out = [0_u8; 32];
            out.copy_from_slice(&bytes);
            if NostrSigner::from_secret(out).is_ok() {
                return Ok(out);
            }
        }

        if let Ok(content) = String::from_utf8(bytes) {
            if let Some(key) = decode_nostr_secret_input(&content) {
                return Ok(key);
            }
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
pub struct MetricsState {
    pub ticks: AtomicU64,
    pub delivered: AtomicU64,
    pub send_failures: AtomicU64,
    pub ack_clears: AtomicU64,
    pub last_fast_outbound_ok: AtomicU64,
    pub last_fast_outbound_err: AtomicU64,
    pub last_fallback_outbound_ok: AtomicU64,
    pub last_fallback_outbound_err: AtomicU64,
    pub last_fast_inbound: AtomicU64,
    pub last_fallback_inbound: AtomicU64,
    pub nostr_bridge_events_total: AtomicU64,
    pub nostr_bridge_payload_bytes_total: AtomicU64,
    pub nostr_bridge_enabled: AtomicU64,
    pub nostr_bridge_relays_configured: AtomicU64,
}

#[derive(Debug)]
pub struct AdminAuthState {
    pub server_pubkey: [u8; 32],
    pub server_pubkey_hex: String,
    pub server_secret_hex: String,
    pub server_secret_nsec: String,
    pub session_ttl_secs: u64,
    pub session_db_path: PathBuf,
    pub settings_db_path: PathBuf,
    pub sessions: Mutex<HashMap<String, u64>>,
}

#[derive(Debug, Deserialize)]
pub struct AdminLoginRequest {
    pub secret: String,
}

#[derive(Debug, Deserialize)]
pub struct AdminSettingUpsertRequest {
    pub key: String,
    pub value: String,
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

    pub fn persist_session_remove(&self, token: &str) {
        if let Ok(conn) = Connection::open(&self.session_db_path) {
            let _ = conn.execute(
                "DELETE FROM admin_sessions WHERE token = ?1",
                params![token],
            );
        }
    }

    pub fn persist_expired_prune(&self, now: u64) {
        if let Ok(conn) = Connection::open(&self.session_db_path) {
            let _ = conn.execute(
                "DELETE FROM admin_sessions WHERE expires_at <= ?1",
                params![now as i64],
            );
        }
    }

    pub fn add_session(&self, token: String, expires: u64) {
        self.sessions
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(token.clone(), expires);
        self.persist_session_insert(&token, expires);
    }

    pub fn revoke_session(&self, token: &str) -> bool {
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

pub fn now_unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub fn decode_nostr_secret_input(value: &str) -> Option<[u8; 32]> {
    let trimmed = value.trim();
    if let Ok(bytes) = hex::decode(trimmed) {
        if let Ok(key) = <[u8; 32]>::try_from(bytes.as_slice()) {
            return Some(key);
        }
    }
    let (decoded_hrp, data) = bech32::decode(trimmed).ok()?;
    if decoded_hrp.as_str() != "nsec" {
        return None;
    }
    let data8 = convert_bits(&data, 5, 8, false)?;
    if let Ok(key) = <[u8; 32]>::try_from(data8.as_slice()) {
        return Some(key);
    }
    None
}

fn encode_nostr_nsec(secret: [u8; 32]) -> Option<String> {
    let hrp = Hrp::parse("nsec").ok()?;
    let data5 = convert_bits(&secret, 8, 5, true)?;
    bech32::encode::<Bech32>(hrp, &data5).ok()
}

fn convert_bits(data: &[u8], from: u32, to: u32, pad: bool) -> Option<Vec<u8>> {
    let mut acc = 0u32;
    let mut bits = 0u32;
    let mut res = Vec::new();
    let maxv = (1u32 << to) - 1;
    for &value in data {
        acc = (acc << from) | (value as u32);
        bits += from;
        while bits >= to {
            bits -= to;
            res.push(((acc >> bits) & maxv) as u8);
        }
    }
    if pad {
        if bits > 0 {
            res.push(((acc << (to - bits)) & maxv) as u8);
        }
    } else if bits >= from || ((acc << (to - bits)) & maxv) != 0 {
        return None;
    }
    Some(res)
}

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Path to configuration file
    #[arg(long, short)]
    config: Option<PathBuf>,
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Run the VPS node (default)
    Run,
    /// Manage node settings
    Settings {
        /// Path to settings database
        #[arg(long, default_value = "data/settings.db")]
        db: PathBuf,
        #[command(subcommand)]
        action: SettingsCommands,
    },
    /// Export node identity (nsec)
    Identity,
}

#[derive(Subcommand)]
enum SettingsCommands {
    /// List all settings
    List,
    /// Get a specific setting
    Get { key: String },
    /// Set a setting value
    Set { key: String, value: String },
    /// Delete a setting
    Delete { key: String },
}

#[tokio::main]
async fn main() {
    let filter = std::env::var("VEIL_LOG").unwrap_or_else(|_| "info".to_string());
    tracing_subscriber::fmt().with_env_filter(filter).init();

    let cli = Cli::parse();

    match &cli.command {
        Some(Commands::Settings { db, action }) => {
            let store = match SettingsStore::open(db) {
                Ok(store) => store,
                Err(err) => {
                    error!("settings db open failed: {err}");
                    std::process::exit(1);
                }
            };

            match action {
                SettingsCommands::List => match store.list() {
                    Ok(items) => {
                        for (k, v) in items {
                            println!("{k}={v}");
                        }
                    }
                    Err(err) => {
                        error!("{err}");
                        std::process::exit(1);
                    }
                },
                SettingsCommands::Get { key } => {
                    if let Some(v) = store.get(key) {
                        println!("{v}");
                    } else {
                        std::process::exit(3);
                    }
                }
                SettingsCommands::Set { key, value } => {
                    if let Err(err) = store.set(key, value.trim()) {
                        error!("{err}");
                        std::process::exit(1);
                    }
                    println!("ok");
                }
                SettingsCommands::Delete { key } => match store.delete(key) {
                    Ok(true) => println!("deleted"),
                    Ok(false) => std::process::exit(3),
                    Err(err) => {
                        error!("{err}");
                        std::process::exit(1);
                    }
                },
            }
            return;
        }
        _ => {}
    }

    let config = match VpsConfig::new(cli.config) {
        Ok(cfg) => cfg,
        Err(err) => {
            error!("failed to load config: {err}");
            std::process::exit(1);
        }
    };

    let settings_db_path = PathBuf::from("data/settings.db");

    let raw_alpn = &config.quic_alpn;
    if !raw_alpn.trim().is_empty() {
        std::env::set_var("VEIL_QUIC_ALPN", raw_alpn);
        info!("quic: using VEIL_VPS_QUIC_ALPN from config: {raw_alpn}");
    }

    let state_path = config.state_path.clone();
    let node_key_path = config.node_key_path.clone();
    let quic_cert_path = config.quic_cert_path.clone();
    let quic_key_path = config.quic_key_path.clone();
    let snapshot_interval = config.snapshot_interval;
    let tick_interval = config.tick_interval;
    let health_bind = config.health_bind.clone();
    let health_port = config.health_port;
    let admin_session_db_path = config.admin_session_db_path.clone();
    let fast_peers = config.fast_peers.clone();
    let core_tags = config.core_tags.clone();
    let tor_peers = config.tor_peers.clone();
    #[cfg(feature = "ble")]
    let ble_enabled = config.ble_enabled;
    #[cfg(feature = "ble")]
    let ble_peers = config.ble_peers.clone();
    #[cfg(feature = "ble")]
    let ble_allowlist = config.ble_allowlist.clone();
    #[cfg(feature = "ble")]
    let ble_mtu = config.ble_mtu;
    let peer_db_path = config.peer_db_path.clone();
    let max_dynamic_peers = config.max_dynamic_peers;

    let quic_bind = config.quic_bind.clone();
    let ws_url = config.ws_url.clone();
    let ws_listen = config.ws_listen.clone();
    let ws_peer = config.ws_peer.clone();
    let ws_peer_id = ws_peer.clone().unwrap_or_else(|| "ws-peer".to_string());
    let tor_socks_addr = config.tor_socks_addr.clone();

    let adaptive_scoring = config.adaptive_lane_scoring;
    let probabilistic_forwarding = config.probabilistic_forwarding;
    let forwarding_min_probability = config.forwarding_min_probability;
    let forwarding_replica_divisor = config.forwarding_replica_divisor;
    let bloom_exchange = config.bloom_exchange;
    let bloom_interval_steps = config.bloom_interval_steps;
    let bloom_false_positive_rate = config.bloom_false_positive_rate;
    let max_cache_shards = config.max_cache_shards;
    let bucket_jitter = config.bucket_jitter;
    let open_relay = config.open_relay;
    let blocked_peers = config.blocked_peers.clone();
    let nostr_bridge_enabled = config.nostr_bridge_enabled;
    let nostr_bridge_relays = config.nostr_bridge_relays.clone();
    let nostr_bridge_channel = config.nostr_bridge_channel_id.clone();
    let nostr_bridge_namespace = config.nostr_bridge_namespace as u16;
    let nostr_bridge_since = config.nostr_bridge_since;
    let nostr_bridge_state_path = config.nostr_bridge_state_path.clone();
    let nostr_bridge_max_seen = config.nostr_bridge_max_seen_ids;
    let nostr_bridge_persist_every = config.nostr_bridge_persist_every_updates;
    let required_signed = parse_required_signed_namespaces(&config.required_signed_namespaces);

    let node_key = match load_or_create_node_key(&node_key_path) {
        Ok(key) => key,
        Err(err) => {
            error!("fatal: {err}");
            return;
        }
    };
    let node_signer = NostrSigner::from_secret(node_key).expect("node key validated");
    let node_pubkey = node_signer.public_key();
    let node_secret_hex = hex::encode(node_key);
    let node_secret_nsec = encode_nostr_nsec(node_key).unwrap_or_default();
    let node_pubkey_hex = hex::encode(node_pubkey);
    info!("node identity (nostr x-only pubkey): {node_pubkey_hex}");

    if let Some(Commands::Identity) = &cli.command {
        println!("nsec: {node_secret_nsec}");
        println!("hex:  {node_secret_hex}");
        return;
    }

    let identity = match load_or_create_identity(&quic_cert_path, &quic_key_path) {
        Ok(identity) => identity,
        Err(err) => {
            error!("fatal: {err}");
            return;
        }
    };

    let trusted_cert_paths = config.quic_trusted_certs.clone();
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
        info!("auto-subscribed to {added} core tags");
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
        info!("open relay mode enabled: accepting all tags and full non-blocked forwarding");
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
            error!("fatal: quic adapter failed to start: {err}");
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
        info!("websocket server listening on {addr}");
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
                error!("ble adapter failed to start: {err:?}");
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
    let mut nostr_bridge_rx = if nostr_bridge_enabled {
        if nostr_bridge_relays.is_empty() {
            warn!("nostr bridge enabled but VEIL_VPS_NOSTR_RELAYS is empty; bridge not started");
            None
        } else {
            info!(
                "nostr bridge enabled with {} relays, channel={}, namespace={}",
                nostr_bridge_relays.len(),
                nostr_bridge_channel,
                nostr_bridge_namespace
            );
            Some(start_nostr_bridge(NostrBridgeConfig {
                relays: nostr_bridge_relays.clone(),
                channel_id: nostr_bridge_channel.clone(),
                namespace: nostr_bridge_namespace,
                since: nostr_bridge_since,
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
        info!(
            "admin auth: restored {} active sessions from {}",
            restored_sessions.len(),
            admin_session_db_path.display()
        );
    }
    let admin_auth = Arc::new(AdminAuthState {
        server_pubkey: node_pubkey,
        server_pubkey_hex: node_pubkey_hex.clone(),
        server_secret_hex: node_secret_hex,
        server_secret_nsec: node_secret_nsec,
        session_ttl_secs: 24 * 60 * 60,
        session_db_path: admin_session_db_path,
        settings_db_path: settings_db_path.clone(),
        sessions: Mutex::new(restored_sessions),
    });
    if health_port != 0 {
        let app_state = http_server::VpsAppState {
            metrics: Arc::clone(&metrics),
            peer_snapshot: Arc::clone(&peer_snapshot),
            admin_auth: Arc::clone(&admin_auth),
            shutdown: Arc::clone(&shutdown),
        };
        let router = http_server::build_router(app_state);
        let bind_addr: std::net::SocketAddr =
            format!("{health_bind}:{health_port}").parse().unwrap();
        let listener = match tokio::net::TcpListener::bind(bind_addr).await {
            Ok(listener) => listener,
            Err(err) => {
                error!("health server bind failed on {bind_addr}: {err}");
                return;
            }
        };
        tokio::spawn(async move {
            if let Err(err) = axum::serve(listener, router).await {
                error!("health server error: {err}");
            }
        });
    }

    let mut now_step = 0_u64;
    loop {
        if shutdown.load(Ordering::Relaxed) {
            if let Err(err) = save_state_to_path(&state_path, &mut runtime.state) {
                error!("snapshot failed on shutdown: {err}");
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

        if let Some(rx) = &mut nostr_bridge_rx {
            for _ in 0..64 {
                match rx.try_recv() {
                    Ok(item) => {
                        info!(
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
                    Err(_) => break,
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
            if let Err(err) = save_state_to_path(&state_path, &mut runtime.state) {
                error!("snapshot failed: {err}");
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
            info!(
                "fast_lane: {:?}, fallback_lane: {:?}",
                health.fast_lane, health.fallback_lane
            );
            last_health_log = Instant::now();
        }

        tokio::time::sleep(tick_interval).await;
    }
}

#[cfg(test)]
mod tests {
    use super::{
        encode_fallback_peers, merge_peers, parse_fallback_peer_strings, Cli, Commands,
        FallbackPeer, SettingsCommands,
    };

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
    fn merge_peers_deduplicates_and_caps() {
        let configured = vec!["a".to_string(), "b".to_string(), "a".to_string()];
        let discovered = vec![
            "b".to_string(),
            "c".to_string(),
            "d".to_string(),
            "e".to_string(),
        ];
        let merged = merge_peers(&configured, &discovered, 4);
        assert_eq!(merged, vec!["a", "b", "c", "d"]);
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

    #[test]
    fn test_cli_parsing() {
        use clap::Parser;

        // Test 'run' (implicit)
        let cli = Cli::try_parse_from(&["veil-vps-node"]).unwrap();
        assert!(cli.command.is_none());

        // Test 'run' (explicit)
        let cli = Cli::try_parse_from(&["veil-vps-node", "run"]).unwrap();
        match cli.command {
            Some(Commands::Run) => {}
            _ => panic!("expected Run command"),
        }

        // Test 'settings'
        let cli = Cli::try_parse_from(&["veil-vps-node", "settings", "list"]).unwrap();
        match cli.command {
            Some(Commands::Settings {
                action: SettingsCommands::List,
                ..
            }) => {}
            _ => panic!("expected Settings List command"),
        }

        // Test 'settings' with custom DB
        let cli = Cli::try_parse_from(&[
            "veil-vps-node",
            "settings",
            "--db",
            "custom.db",
            "get",
            "key",
        ])
        .unwrap();
        match cli.command {
            Some(Commands::Settings {
                ref db,
                action: SettingsCommands::Get { ref key },
            }) => {
                assert_eq!(db, &std::path::PathBuf::from("custom.db"));
                assert_eq!(key, "key");
            }
            _ => panic!("expected Settings Get command"),
        }
    }
}
