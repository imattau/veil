use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use veil_android_node::{serve, AppState, NodeState};

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

    let state = AppState {
        node: NodeState::new(env!("CARGO_PKG_VERSION")),
        auth_token: token,
        version: env!("CARGO_PKG_VERSION").to_string(),
    };

    let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port);
    serve(addr, state).await;
}
