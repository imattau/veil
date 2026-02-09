use std::net::SocketAddr;

use axum::{
    extract::{State, WebSocketUpgrade},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use tracing::info;

use crate::api::{
    EventEnvelope, PublishRequest, PublishResponse, StatusResponse, SubscribeRequest,
    SubscribeResponse, UnsubscribeRequest, UnsubscribeResponse,
};
use crate::state::NodeState;

#[derive(Clone)]
pub struct AppState {
    pub node: NodeState,
    pub auth_token: String,
    pub version: String,
}

pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/status", get(status))
        .route("/publish", post(publish))
        .route("/subscribe", post(subscribe))
        .route("/unsubscribe", post(unsubscribe))
        .route("/events", get(events_ws))
        .with_state(state)
}

pub async fn serve(addr: SocketAddr, state: AppState) {
    let app = build_router(state);
    info!(%addr, "android node listening");
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("bind address");
    axum::serve(listener, app).await.expect("serve");
}

async fn status(State(state): State<AppState>, headers: HeaderMap) -> Response {
    if !authorized(&headers, &state.auth_token) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let payload: StatusResponse = state.node.status();
    Json(payload).into_response()
}

async fn publish(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(_request): Json<PublishRequest>,
) -> Response {
    if !authorized(&headers, &state.auth_token) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let message_id = state.node.enqueue_publish();
    let response = PublishResponse {
        message_id,
        queued: true,
    };
    Json(response).into_response()
}

async fn subscribe(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<SubscribeRequest>,
) -> Response {
    if !authorized(&headers, &state.auth_token) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let changed = state.node.subscribe(&request.tag);
    Json(SubscribeResponse { subscribed: changed }).into_response()
}

async fn unsubscribe(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<UnsubscribeRequest>,
) -> Response {
    if !authorized(&headers, &state.auth_token) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let changed = state.node.unsubscribe(&request.tag);
    Json(UnsubscribeResponse { unsubscribed: changed }).into_response()
}

async fn events_ws(
    State(state): State<AppState>,
    headers: HeaderMap,
    ws: WebSocketUpgrade,
) -> Response {
    if !authorized(&headers, &state.auth_token) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    ws.on_upgrade(move |mut socket| async move {
        let mut rx = state.node.subscribe_events();
        let event = EventEnvelope {
            event: "node_status".to_string(),
            data: serde_json::to_value(state.node.status()).unwrap_or_default(),
        };
        let _ = socket
            .send(axum::extract::ws::Message::Text(
                serde_json::to_string(&event).unwrap_or_default(),
            ))
            .await;
        while let Ok(event) = rx.recv().await {
            let payload = match serde_json::to_string(&event) {
                Ok(value) => value,
                Err(_) => continue,
            };
            if socket
                .send(axum::extract::ws::Message::Text(payload))
                .await
                .is_err()
            {
                break;
            }
        }
    })
}

fn authorized(headers: &HeaderMap, token: &str) -> bool {
    if token.is_empty() {
        return true;
    }
    headers
        .get("x-veil-token")
        .and_then(|value| value.to_str().ok())
        .map(|value| value == token)
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::{Body, Bytes};
    use http::{Request, StatusCode};
    use tower::ServiceExt;

    fn test_state() -> AppState {
        AppState {
            node: NodeState::new("0.1-test"),
            auth_token: "secret".to_string(),
            version: "0.1-test".to_string(),
        }
    }

    #[tokio::test]
    async fn rejects_missing_token() {
        let app = build_router(test_state());
        let response = app
            .oneshot(Request::builder().uri("/status").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn returns_status_with_token() {
        let app = build_router(test_state());
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/status")
                    .header("x-veil-token", "secret")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn publish_queues_message() {
        let app = build_router(test_state());
        let body = serde_json::to_string(&PublishRequest {
            namespace: 32,
            payload: "hello".to_string(),
        })
        .unwrap();
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/publish")
                    .method("POST")
                    .header("content-type", "application/json")
                    .header("x-veil-token", "secret")
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn status_reflects_publish_queue() {
        let app = build_router(test_state());
        let body = serde_json::to_string(&PublishRequest {
            namespace: 32,
            payload: "hello".to_string(),
        })
        .unwrap();
        let _ = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/publish")
                    .method("POST")
                    .header("content-type", "application/json")
                    .header("x-veil-token", "secret")
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/status")
                    .header("x-veil-token", "secret")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap_or_else(|_| Bytes::new());
        let parsed: StatusResponse = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(parsed.queue.pending, 1);
    }
}
