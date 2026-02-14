use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;

use std::path::PathBuf;
use std::sync::Arc;

use veil_node::config::{BloomExchangeConfig, ProbabilisticForwardingConfig};

use veil_android_node::{
    build_self_contact, default_protocol_config, serve, AppState, DiscoveryConfig, DiscoveryWorker,
    LanDiscoveryConfig, LanDiscoveryWorker, NodeState, ProtocolEngine, QueueWorker,
    QueueWorkerConfig,
};

#[tokio::main]
async fn main() {
    let filter = std::env::var("VEIL_NODE_LOG_LEVEL").unwrap_or_else(|_| "info".to_string());
    tracing_subscriber::fmt().with_env_filter(filter).init();

    let token = std::env::var("VEIL_NODE_TOKEN")
        .unwrap_or_default()
        .trim()
        .to_string();
    let token = if token.is_empty() {
        let generated = uuid::Uuid::new_v4().to_string();
        tracing::warn!(
            "VEIL_NODE_TOKEN was not set; generated an ephemeral token for this process"
        );
        generated
    } else {
        token
    };
    let allow_identity_export = env_bool("VEIL_NODE_ALLOW_IDENTITY_EXPORT", false);
    let host = std::env::var("VEIL_NODE_HOST")
        .ok()
        .and_then(|value| value.parse::<IpAddr>().ok())
        .unwrap_or(IpAddr::V4(Ipv4Addr::LOCALHOST));
    let port = std::env::var("VEIL_NODE_PORT")
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(7788);

    let store_path = std::env::var("VEIL_NODE_STATE").map(PathBuf::from).ok();
    let node = NodeState::new_with_store(env!("CARGO_PKG_VERSION"), store_path);
    let node_arc = Arc::new(node.clone());
    let identity = node.identity();

    let ws_url = std::env::var("VEIL_NODE_WS").ok();
    let peer_id = std::env::var("VEIL_NODE_PEER").unwrap_or_else(|_| "android-node".to_string());
    let namespace = std::env::var("VEIL_NODE_NAMESPACE")
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(32);
    let mut protocol_config = default_protocol_config(
        ws_url.clone().unwrap_or_default(),
        peer_id,
        namespace,
        identity.public_key,
        identity.encrypt_key,
        identity.signer(),
    );
    if ws_url.is_none() {
        protocol_config.ws_url = None;
    }
    protocol_config.runtime_config.wot_policy = node.wot_policy();
    if let Ok(raw) = std::env::var("VEIL_NODE_CACHE_STATE") {
        if !raw.trim().is_empty() {
            protocol_config.cache_state_path = Some(PathBuf::from(raw));
        }
    }
    if let Ok(raw) = std::env::var("VEIL_NODE_QUIC_PEERS") {
        protocol_config.fast_peers = raw
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
    } else if let Ok(raw) = std::env::var("VEIL_NODE_FAST_PEERS") {
        protocol_config.fast_peers = raw
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
    }
    if let Ok(raw) = std::env::var("VEIL_NODE_WS_PEERS") {
        protocol_config.fallback_peers = raw
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
    } else if let Ok(raw) = std::env::var("VEIL_NODE_FALLBACK_PEERS") {
        protocol_config.fallback_peers = raw
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
    }
    if let Ok(raw) = std::env::var("VEIL_NODE_PEER_PUBKEYS") {
        for entry in raw.split(',') {
            let mut parts = entry.splitn(2, '=');
            let peer = parts.next().unwrap_or("").trim();
            let hex = parts.next().unwrap_or("").trim();
            if peer.is_empty() || hex.len() != 64 {
                continue;
            }
            if let Ok(bytes) = hex::decode(hex) {
                if bytes.len() == 32 {
                    let mut key = [0u8; 32];
                    key.copy_from_slice(&bytes);
                    protocol_config
                        .runtime_config
                        .bind_peer_publisher(peer, key);
                }
            }
        }
    }
    if let Ok(raw) = std::env::var("VEIL_NODE_WS") {
        if !raw.trim().is_empty() {
            protocol_config.ws_url = Some(raw);
        }
    }
    if let Ok(raw) = std::env::var("VEIL_NODE_QUIC_BIND") {
        if !raw.trim().is_empty() {
            protocol_config.quic_bind_addr = raw;
        }
    }
    if let Ok(raw) = std::env::var("VEIL_NODE_QUIC_SERVER_NAME") {
        if !raw.trim().is_empty() {
            protocol_config.quic_server_name = Some(raw);
        }
    }
    if let Ok(raw) = std::env::var("VEIL_NODE_TOR_SOCKS") {
        if !raw.trim().is_empty() {
            protocol_config.tor_socks = Some(raw);
        }
    }
    if let Ok(raw) = std::env::var("VEIL_NODE_QUIC_CERT_HEX") {
        if let Ok(bytes) = hex::decode(raw.trim()) {
            if !bytes.is_empty() {
                protocol_config.quic_trusted_certs = vec![bytes];
            }
        }
    }
    if let Ok(raw) = std::env::var("VEIL_DISCOVERY_NAMESPACE") {
        if let Ok(value) = raw.trim().parse::<u16>() {
            protocol_config.discovery_namespace = veil_core::Namespace(value);
        }
    }
    protocol_config.runtime_config.probabilistic_forwarding = ProbabilisticForwardingConfig {
        enabled: env_bool(
            "VEIL_NODE_PROBABILISTIC_FORWARDING",
            protocol_config
                .runtime_config
                .probabilistic_forwarding
                .enabled,
        ),
        min_probability: env_f64(
            "VEIL_NODE_FORWARD_MIN_PROBABILITY",
            protocol_config
                .runtime_config
                .probabilistic_forwarding
                .min_probability,
        )
        .clamp(0.0, 1.0),
        replica_divisor: env_u64(
            "VEIL_NODE_FORWARD_REPLICA_DIVISOR",
            protocol_config
                .runtime_config
                .probabilistic_forwarding
                .replica_divisor,
        )
        .max(1),
    };
    protocol_config.runtime_config.bloom_exchange = BloomExchangeConfig {
        enabled: env_bool(
            "VEIL_NODE_BLOOM_EXCHANGE",
            protocol_config.runtime_config.bloom_exchange.enabled,
        ),
        interval_steps: env_u64(
            "VEIL_NODE_BLOOM_INTERVAL_STEPS",
            protocol_config.runtime_config.bloom_exchange.interval_steps,
        )
        .max(1),
        false_positive_rate: env_f64(
            "VEIL_NODE_BLOOM_FALSE_POSITIVE_RATE",
            protocol_config
                .runtime_config
                .bloom_exchange
                .false_positive_rate,
        )
        .clamp(0.001, 0.5),
    };
    let protocol = Arc::new(ProtocolEngine::new(protocol_config).expect("protocol engine init"));

    // Sync persisted contacts to ProtocolEngine
    let contacts = node.contacts();
    if !contacts.is_empty() {
        let p = protocol.clone();
        tokio::spawn(async move {
            p.sync_contacts(&contacts).await;
            tracing::info!(
                "Synced {} persisted contacts to protocol engine",
                contacts.len()
            );
        });
    }

    let discovery_config = DiscoveryConfig {
        bootstrap_urls: std::env::var("VEIL_DISCOVERY_BOOTSTRAP")
            .unwrap_or_default()
            .split(',')
            .map(|value| value.trim().to_string())
            .filter(|value| {
                !value.is_empty()
                    && (value.starts_with("http://")
                        || value.starts_with("https://")
                        || value.starts_with("ws://")
                        || value.starts_with("wss://")
                        || value.starts_with("quic://"))
            })
            .collect(),
        gossip_interval: std::env::var("VEIL_DISCOVERY_INTERVAL_MS")
            .ok()
            .and_then(|value| value.parse::<u64>().ok())
            .map(Duration::from_millis)
            .unwrap_or_else(|| Duration::from_secs(12)),
        max_gossip_contacts: std::env::var("VEIL_DISCOVERY_GOSSIP_MAX")
            .ok()
            .and_then(|value| value.parse::<usize>().ok())
            .unwrap_or(24),
        transport_enabled: std::env::var("VEIL_DISCOVERY_TRANSPORT")
            .ok()
            .as_deref()
            .map(|value| value != "0")
            .unwrap_or(true),
    };
    let lan_config = LanDiscoveryConfig {
        enabled: std::env::var("VEIL_LAN_DISCOVERY").ok().as_deref() == Some("1"),
        port: std::env::var("VEIL_LAN_DISCOVERY_PORT")
            .ok()
            .and_then(|value| value.parse::<u16>().ok())
            .unwrap_or(9333),
        announce_interval: std::env::var("VEIL_LAN_DISCOVERY_INTERVAL_MS")
            .ok()
            .and_then(|value| value.parse::<u64>().ok())
            .map(Duration::from_millis)
            .unwrap_or_else(|| Duration::from_secs(5)),
    };

    let worker = QueueWorker::new(
        node_arc,
        protocol.clone(),
        QueueWorkerConfig {
            tick_ms: 500,
            max_attempts: 3,
            backoff_base_ms: 500,
            backoff_max_ms: 20_000,
        },
    );
    tokio::spawn(worker.run());

    let discovery_worker =
        DiscoveryWorker::new(Arc::new(node.clone()), protocol.clone(), discovery_config);
    tokio::spawn(discovery_worker.run());
    let lan_worker = LanDiscoveryWorker::new(Arc::new(node.clone()), protocol.clone(), lan_config);
    let self_contact = build_self_contact(&node, &protocol);
    lan_worker.start(self_contact);

    let state = AppState {
        node,
        protocol,
        auth_token: token,
        allow_identity_export,
        version: env!("CARGO_PKG_VERSION").to_string(),
    };

    let addr = SocketAddr::new(host, port);
    serve(addr, state).await;
}

fn env_bool(key: &str, default: bool) -> bool {
    std::env::var(key)
        .ok()
        .map(|value| matches!(value.as_str(), "1" | "true" | "TRUE" | "yes" | "YES"))
        .unwrap_or(default)
}

fn env_u64(key: &str, default: u64) -> u64 {
    std::env::var(key)
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(default)
}

fn env_f64(key: &str, default: f64) -> f64 {
    std::env::var(key)
        .ok()
        .and_then(|value| value.parse::<f64>().ok())
        .unwrap_or(default)
}
