use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use std::path::PathBuf;
use std::sync::Arc;

use veil_android_node::{serve, AppState, NodeState, QueueWorker, QueueWorkerConfig};

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

    let worker = QueueWorker::new(
        node_arc,
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
