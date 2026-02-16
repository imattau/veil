use super::*;

#[tokio::test]
async fn rejects_missing_token() {
    let app = build_router(test_state());
    let response = app
        .oneshot(
            Request::builder()
                .uri("/status")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn rejects_wrong_token() {
    let app = build_router(test_state());
    let response = app
        .oneshot(
            Request::builder()
                .uri("/status")
                .header("x-veil-token", "wrong")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn health_allows_unauthenticated_access() {
    let app = build_router(test_state());
    let response = app
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap_or_else(|_| Bytes::new());
    let parsed: HealthResponse = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(parsed.status, "ok");
    assert!(!parsed.version.is_empty());
}

#[tokio::test]
async fn publish_rejects_oversized_payload() {
    let app = build_router(test_state());
    let body = serde_json::to_string(&PublishRequest {
        namespace: 32,
        payload: "a".repeat(MAX_RAW_PAYLOAD_BYTES + 1),
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
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn publish_object_returns_fetchable_content_root() {
    let app = build_router(test_state());
    let payload = b"mock-image-bytes".to_vec();
    let body = serde_json::to_string(&ObjectPublishRequest {
        namespace: 32,
        payload_b64: base64::engine::general_purpose::STANDARD.encode(&payload),
    })
    .unwrap();

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/publish_object")
                .method("POST")
                .header("content-type", "application/json")
                .header("x-veil-token", "secret")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap_or_else(|_| Bytes::new());
    let parsed: ObjectPublishResponse = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(parsed.object_root.len(), 64);
    assert_eq!(parsed.wire_root.len(), 64);
    assert_ne!(parsed.object_root, parsed.wire_root);

    let object_response = app
        .oneshot(
            Request::builder()
                .uri(format!("/object/{}", parsed.object_root))
                .header("x-veil-token", "secret")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(object_response.status(), StatusCode::OK);

    let object_bytes = axum::body::to_bytes(object_response.into_body(), usize::MAX)
        .await
        .unwrap_or_else(|_| Bytes::new());
    let object_payload: ObjectFetchResponse = serde_json::from_slice(&object_bytes).unwrap();
    let fetched = base64::engine::general_purpose::STANDARD
        .decode(object_payload.object_b64)
        .unwrap();
    assert_eq!(fetched, payload);
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

#[test]
fn empty_token_never_authorizes() {
    let headers = HeaderMap::new();
    assert!(!authorized(&headers, ""));
}

#[tokio::test]
async fn identity_export_is_disabled_by_default() {
    let app = build_router(test_state());
    let response = app
        .oneshot(
            Request::builder()
                .uri("/identity/export")
                .header("x-veil-token", "secret")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn identity_export_returns_secret_when_enabled() {
    let app = build_router(test_state_with_export_enabled());
    let response = app
        .oneshot(
            Request::builder()
                .uri("/identity/export")
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
    let parsed: IdentityExportResponse = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(parsed.public_key_hex.len(), 64);
    assert_eq!(parsed.secret_key_hex.len(), 64);
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

#[tokio::test]
async fn publish_response_contains_message_id() {
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
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap_or_else(|_| Bytes::new());
    let parsed: PublishResponse = serde_json::from_slice(&bytes).unwrap();
    assert!(!parsed.message_id.is_nil());
}
