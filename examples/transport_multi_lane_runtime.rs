use std::env;
use std::time::Duration;

use veil_crypto::aead::XChaCha20Poly1305Cipher;
use veil_crypto::signing::Ed25519Verifier;
use veil_node::config::NodeRuntimeConfig;
use veil_node::service::{NodeRuntime, NodeRuntimeRunnerConfig};
use veil_transport_tor::{TorSocksAdapter, TorSocksAdapterConfig};
use veil_transport_websocket::{WebSocketAdapter, WebSocketAdapterConfig};

#[derive(Debug, Clone)]
struct TransportProfileConfig {
    fast_ws_url: String,
    fallback_socks_proxy: String,
    fast_peers: Vec<String>,
    fallback_peers: Vec<String>,
}

impl TransportProfileConfig {
    fn from_env() -> Self {
        let fast_ws_url =
            env::var("VEIL_FAST_WS_URL").unwrap_or_else(|_| "ws://127.0.0.1:9001".to_string());
        let fallback_socks_proxy =
            env::var("VEIL_FALLBACK_SOCKS_PROXY").unwrap_or_else(|_| "127.0.0.1:9050".to_string());
        let fast_peers = env::var("VEIL_FAST_PEERS")
            .unwrap_or_else(|_| "fast-peer".to_string())
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>();
        let fallback_peers = env::var("VEIL_FALLBACK_PEERS")
            .unwrap_or_else(|_| "fallback-peer".to_string())
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>();

        Self {
            fast_ws_url,
            fallback_socks_proxy,
            fast_peers,
            fallback_peers,
        }
    }
}

fn main() {
    let cfg = TransportProfileConfig::from_env();
    let node_cfg = NodeRuntimeConfig::edge_forwarder_hot_cache_defaults();
    let key = [0xA5_u8; 32];

    let fast_adapter = WebSocketAdapter::connect(WebSocketAdapterConfig {
        url: cfg.fast_ws_url.clone(),
        peer_id: "fast-lane".to_string(),
        reconnect: true,
        reconnect_initial: Duration::from_millis(250),
        reconnect_max: Duration::from_secs(10),
        outbound_queue_capacity: 1024,
        inbound_queue_capacity: 4096,
        max_payload_hint: Some(64 * 1024),
    })
    .expect("fast websocket adapter should initialize");

    let fallback_adapter = TorSocksAdapter::connect(TorSocksAdapterConfig {
        socks_proxy_addr: cfg.fallback_socks_proxy.clone(),
        connect_timeout: Duration::from_secs(8),
        send_timeout: Duration::from_secs(8),
        outbound_queue_capacity: 1024,
        max_payload_hint: Some(64 * 1024),
    })
    .expect("fallback tor adapter should initialize");

    let mut runtime = NodeRuntime::new(
        veil_node::state::NodeState::default(),
        fast_adapter,
        fallback_adapter,
        node_cfg,
        key,
        XChaCha20Poly1305Cipher,
        Ed25519Verifier,
    );

    let exit = runtime.run_steps(
        20,
        &cfg.fast_peers,
        &cfg.fallback_peers,
        NodeRuntimeRunnerConfig {
            start_step: 0,
            tick_interval: Duration::from_millis(50),
            error_backoff: Duration::from_millis(250),
            max_consecutive_errors: Some(16),
        },
        None,
    );

    println!("transport_multi_lane_runtime complete: {exit:?}");
}
