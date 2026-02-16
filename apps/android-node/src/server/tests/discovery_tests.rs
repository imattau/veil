use super::*;

#[tokio::test]
async fn discovery_announce_returns_neighbors() {
    let state = test_state();
    let app = build_router(state.clone());
    let contact = ContactBundle {
        peer_id: "peer-b".to_string(),
        ws_url: None,
        quic_addr: Some("127.0.0.1:9002".to_string()),
        pubkey_hex: "bb".repeat(32),
        rpc_url: None,
        lan_addrs: Vec::new(),
    };
    let body = serde_json::to_string(&DiscoveryAnnounceRequest { contact }).unwrap();
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/discovery/announce")
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
    let parsed: DiscoveryAnnounceResponse = serde_json::from_slice(&bytes).unwrap();
    assert!(parsed.accepted);
    let (fast, _) = state.protocol.dynamic_peer_snapshot().await;
    assert!(
        fast.iter().any(|peer| peer == "127.0.0.1:9002"),
        "announced peer should be registered for transport"
    );
}

#[tokio::test]
async fn discovery_announce_rejects_invalid_contact() {
    let app = build_router(test_state());
    let contact = ContactBundle {
        peer_id: "peer-b".to_string(),
        ws_url: None,
        quic_addr: Some("127.0.0.1:9002".to_string()),
        pubkey_hex: "z".repeat(64),
        rpc_url: None,
        lan_addrs: Vec::new(),
    };
    let body = serde_json::to_string(&DiscoveryAnnounceRequest { contact }).unwrap();
    let response = app
        .oneshot(
            Request::builder()
                .uri("/discovery/announce")
                .method("POST")
                .header("content-type", "application/json")
                .header("x-veil-token", "secret")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap_or_else(|_| Bytes::new());
    let parsed: ErrorResponse = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(parsed.code, "invalid_contact");
}

#[tokio::test]
async fn discovery_lookup_returns_contacts() {
    let app = build_router(test_state());
    let contact = ContactBundle {
        peer_id: "peer-c".to_string(),
        ws_url: None,
        quic_addr: Some("127.0.0.1:9003".to_string()),
        pubkey_hex: "cc".repeat(32),
        rpc_url: None,
        lan_addrs: Vec::new(),
    };
    let announce = serde_json::to_string(&DiscoveryAnnounceRequest { contact }).unwrap();
    let _ = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/discovery/announce")
                .method("POST")
                .header("content-type", "application/json")
                .header("x-veil-token", "secret")
                .body(Body::from(announce))
                .unwrap(),
        )
        .await
        .unwrap();
    let lookup = serde_json::to_string(&DiscoveryLookupRequest {
        peer_id: Some("peer-c".to_string()),
        pubkey_hex: None,
        limit: Some(4),
    })
    .unwrap();
    let response = app
        .oneshot(
            Request::builder()
                .uri("/discovery/lookup")
                .method("POST")
                .header("content-type", "application/json")
                .header("x-veil-token", "secret")
                .body(Body::from(lookup))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap_or_else(|_| Bytes::new());
    let parsed: DiscoveryLookupResponse = serde_json::from_slice(&bytes).unwrap();
    assert!(!parsed.contacts.is_empty());
}

#[tokio::test]
async fn discovery_lookup_rejects_empty_query() {
    let app = build_router(test_state());
    let lookup = serde_json::to_string(&DiscoveryLookupRequest {
        peer_id: None,
        pubkey_hex: None,
        limit: Some(4),
    })
    .unwrap();
    let response = app
        .oneshot(
            Request::builder()
                .uri("/discovery/lookup")
                .method("POST")
                .header("content-type", "application/json")
                .header("x-veil-token", "secret")
                .body(Body::from(lookup))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap_or_else(|_| Bytes::new());
    let parsed: ErrorResponse = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(parsed.code, "invalid_lookup");
}

#[tokio::test]
async fn discovery_lookup_rejects_invalid_pubkey() {
    let app = build_router(test_state());
    let lookup = serde_json::to_string(&DiscoveryLookupRequest {
        peer_id: None,
        pubkey_hex: Some("nothex".to_string()),
        limit: Some(4),
    })
    .unwrap();
    let response = app
        .oneshot(
            Request::builder()
                .uri("/discovery/lookup")
                .method("POST")
                .header("content-type", "application/json")
                .header("x-veil-token", "secret")
                .body(Body::from(lookup))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap_or_else(|_| Bytes::new());
    let parsed: ErrorResponse = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(parsed.code, "invalid_pubkey");
}
