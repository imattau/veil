use std::net::SocketAddr;

use tracing::info;

use super::{build_router, AppState};

pub async fn serve(addr: SocketAddr, state: AppState) {
    let app = build_router(state);
    info!(%addr, "android node listening");
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("bind address");
    axum::serve(listener, app).await.expect("serve");
}
