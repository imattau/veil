use super::*;

#[tokio::test]
async fn contact_import_adds_contact() {
    let app = build_router(test_state());
    let contact = ContactBundle {
        peer_id: "peer-a".to_string(),
        ws_url: Some("ws://127.0.0.1:9001/ws".to_string()),
        quic_addr: Some("127.0.0.1:9000".to_string()),
        pubkey_hex: "aa".repeat(32),
        rpc_url: Some("http://127.0.0.1:7788".to_string()),
        lan_addrs: vec!["192.168.1.5:9000".to_string()],
    };
    let body = serde_json::to_string(&ContactImportRequest { contact }).unwrap();
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/contact")
                .method("POST")
                .header("content-type", "application/json")
                .header("x-veil-token", "secret")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/contact")
                .header("x-veil-token", "secret")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap_or_else(|_| Bytes::new());
    let parsed: ContactListResponse = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(parsed.contacts.len(), 1);
}

#[tokio::test]
async fn contact_import_replaces_existing_contact() {
    let app = build_router(test_state());
    let original = ContactBundle {
        peer_id: "peer-a".to_string(),
        ws_url: Some("ws://127.0.0.1:9001/ws".to_string()),
        quic_addr: Some("127.0.0.1:9000".to_string()),
        pubkey_hex: "11".repeat(32),
        rpc_url: None,
        lan_addrs: Vec::new(),
    };
    let body = serde_json::to_string(&ContactImportRequest { contact: original }).unwrap();
    let _ = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/contact")
                .method("POST")
                .header("content-type", "application/json")
                .header("x-veil-token", "secret")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    let updated = ContactBundle {
        peer_id: "peer-a".to_string(),
        ws_url: Some("ws://example.com/ws".to_string()),
        quic_addr: Some("198.51.100.4:5000".to_string()),
        pubkey_hex: "22".repeat(32),
        rpc_url: Some("https://example.com/rpc".to_string()),
        lan_addrs: vec!["192.168.1.10:5000".to_string()],
    };
    let body = serde_json::to_string(&ContactImportRequest { contact: updated }).unwrap();
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/contact")
                .method("POST")
                .header("content-type", "application/json")
                .header("x-veil-token", "secret")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/contact")
                .header("x-veil-token", "secret")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap_or_else(|_| Bytes::new());
    let parsed: ContactListResponse = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(parsed.contacts.len(), 1);
    let only = &parsed.contacts[0];
    assert_eq!(only.peer_id, "peer-a");
    assert_eq!(only.ws_url.as_deref(), Some("ws://example.com/ws"));
    assert_eq!(only.quic_addr.as_deref(), Some("198.51.100.4:5000"));
    assert_eq!(only.pubkey_hex, "22".repeat(32));
}

#[tokio::test]
async fn contact_delete_removes_contact() {
    let app = build_router(test_state());
    let contact = ContactBundle {
        peer_id: "peer-delete".to_string(),
        ws_url: Some("ws://127.0.0.1:9001/ws".to_string()),
        quic_addr: Some("127.0.0.1:9000".to_string()),
        pubkey_hex: "33".repeat(32),
        rpc_url: None,
        lan_addrs: Vec::new(),
    };
    let body = serde_json::to_string(&ContactImportRequest { contact }).unwrap();
    let _ = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/contact")
                .method("POST")
                .header("content-type", "application/json")
                .header("x-veil-token", "secret")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    let body = serde_json::to_string(&ContactDeleteRequest {
        peer_id: "peer-delete".to_string(),
    })
    .unwrap();
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/contact/delete")
                .method("POST")
                .header("content-type", "application/json")
                .header("x-veil-token", "secret")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/contact")
                .header("x-veil-token", "secret")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap_or_else(|_| Bytes::new());
    let parsed: ContactListResponse = serde_json::from_slice(&bytes).unwrap();
    assert!(parsed.contacts.is_empty());
}
