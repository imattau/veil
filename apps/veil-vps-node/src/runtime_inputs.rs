use std::path::PathBuf;
use std::time::Duration;

use crate::config::VpsConfig;

pub(super) struct RuntimeInputs {
    pub quic_alpn: String,
    pub state_path: PathBuf,
    pub node_key_path: PathBuf,
    pub node_key_input: Option<String>,
    pub quic_cert_path: PathBuf,
    pub quic_key_path: PathBuf,
    pub snapshot_interval: Duration,
    pub tick_interval: Duration,
    pub health_bind: String,
    pub health_port: u16,
    pub admin_session_db_path: PathBuf,
    pub peer_db_path: PathBuf,
    pub max_dynamic_peers: usize,
    pub max_peer_db_rows: usize,
    pub quic_bind: String,
    pub ws_url: Option<String>,
    pub ws_listen: Option<String>,
    pub ws_peer: Option<String>,
    pub ws_peer_id: String,
    pub tor_socks_addr: Option<String>,
    pub fast_peers: Vec<String>,
    pub core_tags: Vec<String>,
    pub tor_peers: Vec<String>,
    #[cfg(feature = "ble")]
    pub ble_enabled: bool,
    #[cfg(feature = "ble")]
    pub ble_peers: Vec<String>,
    #[cfg(feature = "ble")]
    pub ble_allowlist: Vec<String>,
    #[cfg(feature = "ble")]
    pub ble_mtu: usize,
    pub adaptive_scoring: bool,
    pub probabilistic_forwarding: bool,
    pub forwarding_min_probability: f64,
    pub forwarding_replica_divisor: u64,
    pub bloom_exchange: bool,
    pub bloom_interval_steps: u64,
    pub bloom_false_positive_rate: f64,
    pub max_cache_shards: usize,
    pub bucket_jitter: usize,
    pub open_relay: bool,
    pub blocked_peers: Vec<String>,
    pub nostr_bridge_enabled: bool,
    pub nostr_bridge_relays: Vec<String>,
    pub nostr_bridge_channel: String,
    pub nostr_bridge_namespace: u16,
    pub nostr_bridge_since: Duration,
    pub nostr_bridge_state_path: PathBuf,
    pub nostr_bridge_max_seen: usize,
    pub nostr_bridge_persist_every: usize,
    pub required_signed_raw: Vec<String>,
    pub quic_trusted_certs: Vec<String>,
}

impl RuntimeInputs {
    pub(super) fn from_config(config: VpsConfig) -> Self {
        let ws_url = config.ws_url.clone().filter(|s| !s.trim().is_empty());
        let ws_listen = config.ws_listen.clone().filter(|s| !s.trim().is_empty());
        let ws_peer = config.ws_peer.clone().filter(|s| !s.trim().is_empty());
        let ws_peer_id = ws_peer.clone().unwrap_or_else(|| "ws-peer".to_string());
        let max_dynamic_peers = config.max_dynamic_peers;
        let max_peer_db_rows = max_dynamic_peers.saturating_mul(2).max(1);

        Self {
            quic_alpn: config.quic_alpn,
            state_path: config.state_path,
            node_key_path: config.node_key_path,
            node_key_input: config.node_key,
            quic_cert_path: config.quic_cert_path,
            quic_key_path: config.quic_key_path,
            snapshot_interval: config.snapshot_interval,
            tick_interval: config.tick_interval,
            health_bind: config.health_bind,
            health_port: config.health_port,
            admin_session_db_path: config.admin_session_db_path,
            peer_db_path: config.peer_db_path,
            max_dynamic_peers,
            max_peer_db_rows,
            quic_bind: config.quic_bind,
            ws_url,
            ws_listen,
            ws_peer,
            ws_peer_id,
            tor_socks_addr: config.tor_socks_addr.filter(|s| !s.trim().is_empty()),
            fast_peers: config.fast_peers,
            core_tags: config.core_tags,
            tor_peers: config.tor_peers,
            #[cfg(feature = "ble")]
            ble_enabled: config.ble_enabled,
            #[cfg(feature = "ble")]
            ble_peers: config.ble_peers,
            #[cfg(feature = "ble")]
            ble_allowlist: config.ble_allowlist,
            #[cfg(feature = "ble")]
            ble_mtu: config.ble_mtu,
            adaptive_scoring: config.adaptive_lane_scoring,
            probabilistic_forwarding: config.probabilistic_forwarding,
            forwarding_min_probability: config.forwarding_min_probability,
            forwarding_replica_divisor: config.forwarding_replica_divisor,
            bloom_exchange: config.bloom_exchange,
            bloom_interval_steps: config.bloom_interval_steps,
            bloom_false_positive_rate: config.bloom_false_positive_rate,
            max_cache_shards: config.max_cache_shards,
            bucket_jitter: config.bucket_jitter,
            open_relay: config.open_relay,
            blocked_peers: config.blocked_peers,
            nostr_bridge_enabled: config.nostr_bridge_enabled,
            nostr_bridge_relays: config.nostr_bridge_relays,
            nostr_bridge_channel: config.nostr_bridge_channel_id,
            nostr_bridge_namespace: config.nostr_bridge_namespace as u16,
            nostr_bridge_since: config.nostr_bridge_since,
            nostr_bridge_state_path: config.nostr_bridge_state_path,
            nostr_bridge_max_seen: config.nostr_bridge_max_seen_ids,
            nostr_bridge_persist_every: config.nostr_bridge_persist_every_updates,
            required_signed_raw: config.required_signed_namespaces,
            quic_trusted_certs: config.quic_trusted_certs,
        }
    }
}
