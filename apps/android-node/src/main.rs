use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use std::path::PathBuf;
use std::sync::Arc;

use veil_android_node::{
    default_protocol_config, serve, AppState, NodeState, ProtocolEngine, QueueWorker,
    QueueWorkerConfig,
};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .init();

    let token = std::env::var("VEIL_NODE_TOKEN").unwrap_or_default();
    let port = std::env::var("VEIL_NODE_PORT")
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(7788);

    let store_path = std::env::var("VEIL_NODE_STATE")
        .map(PathBuf::from)
        .ok();
    let node = NodeState::new_with_store(env!("CARGO_PKG_VERSION"), store_path);
    let node_arc = Arc::new(node.clone());

    let ws_url = std::env::var("VEIL_NODE_WS").unwrap_or_else(|_| "ws://127.0.0.1:9001/ws".to_string());
    let peer_id = std::env::var("VEIL_NODE_PEER").unwrap_or_else(|_| "android-node".to_string());
    let namespace = std::env::var("VEIL_NODE_NAMESPACE")
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(32);
    let mut protocol_config = default_protocol_config(ws_url, peer_id, namespace);
    if let Ok(raw) = std::env::var("VEIL_NODE_FAST_PEERS") {
        protocol_config.fast_peers = raw
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
    }
    if let Ok(raw) = std::env::var("VEIL_NODE_FALLBACK_PEERS") {
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
                    protocol_config.runtime_config.bind_peer_publisher(peer, key);
                }
            }
        }
    }
    let protocol = Arc::new(
        ProtocolEngine::new(protocol_config).expect("protocol engine init"),
    );

    let worker = QueueWorker::new(
        node_arc,
        protocol,
        QueueWorkerConfig {
            tick_ms: 500,
            max_attempts: 3,
        },
    );
    tokio::spawn(worker.run());

    let state = AppState {
        node,
        auth_token: token,
        version: env!("CARGO_PKG_VERSION").to_string(),
    };

    let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port);
    serve(addr, state).await;
}
