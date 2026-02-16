use super::*;

#[tokio::test]
async fn group_message_publish_accepts_empty_author() {
    let app = build_router(test_state());
    let body = serde_json::to_string(&GroupMessagePublishRequest {
        namespace: 32,
        bundle: GroupMessageBundle {
            meta: BundleMeta {
                version: 1,
                created_at: 1_700_000_034,
            },
            channel_id: "general".to_string(),
            author_pubkey_hex: String::new(),
            group_id: "group-alpha".to_string(),
            ciphertext_root: [0x33; 32],
            reply_to_root: None,
        },
    })
    .unwrap();
    let response = app
        .oneshot(
            Request::builder()
                .uri("/group_message")
                .header("content-type", "application/json")
                .header("x-veil-token", "secret")
                .method("POST")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap_or_else(|_| Bytes::new());
    let parsed: GroupMessagePublishResponse = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(parsed.author_pubkey_hex.len(), 64);
}

#[tokio::test]
async fn group_message_publish_rejects_empty_group_id() {
    let app = build_router(test_state());
    let body = serde_json::to_string(&GroupMessagePublishRequest {
        namespace: 32,
        bundle: GroupMessageBundle {
            meta: BundleMeta {
                version: 1,
                created_at: 1_700_000_035,
            },
            channel_id: "general".to_string(),
            author_pubkey_hex: String::new(),
            group_id: " ".to_string(),
            ciphertext_root: [0x33; 32],
            reply_to_root: None,
        },
    })
    .unwrap();
    let response = app
        .oneshot(
            Request::builder()
                .uri("/group_message")
                .header("content-type", "application/json")
                .header("x-veil-token", "secret")
                .method("POST")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn group_message_text_publish_encrypts_and_queues() {
    let app = build_router(test_state());
    let body = serde_json::to_string(&GroupMessageTextPublishRequest {
        namespace: 32,
        channel_id: "general".to_string(),
        group_id: "group-alpha".to_string(),
        text: "group secret".to_string(),
        reply_to_root: None,
        member_pubkeys: vec![],
    })
    .unwrap();
    let response = app
        .oneshot(
            Request::builder()
                .uri("/group_message_text")
                .header("content-type", "application/json")
                .header("x-veil-token", "secret")
                .method("POST")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn group_key_share_route_accepts_members() {
    let app = build_router(test_state());
    let member_pubkey_hex = hex::encode(
        veil_crypto::signing::NostrSigner::from_secret([4u8; 32])
            .expect("valid secret")
            .public_key(),
    );
    let body = serde_json::to_string(&GroupKeyShareRequest {
        namespace: 32,
        channel_id: "group".to_string(),
        group_id: "group-alpha".to_string(),
        member_pubkeys: vec![member_pubkey_hex],
        rotate_key: true,
    })
    .unwrap();
    let response = app
        .oneshot(
            Request::builder()
                .uri("/group_key/share")
                .header("content-type", "application/json")
                .header("x-veil-token", "secret")
                .method("POST")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap_or_else(|_| Bytes::new());
    let parsed: GroupKeyShareResponse = serde_json::from_slice(&bytes).unwrap();
    assert!(parsed.queued);
}

#[tokio::test]
async fn group_key_share_route_rejects_self_only_members() {
    let state = test_state();
    let self_pubkey_hex = state.node.identity().public_key_hex();
    let app = build_router(state);
    let body = serde_json::to_string(&GroupKeyShareRequest {
        namespace: 32,
        channel_id: "group".to_string(),
        group_id: "group-alpha".to_string(),
        member_pubkeys: vec![self_pubkey_hex],
        rotate_key: false,
    })
    .unwrap();
    let response = app
        .oneshot(
            Request::builder()
                .uri("/group_key/share")
                .header("content-type", "application/json")
                .header("x-veil-token", "secret")
                .method("POST")
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
    assert_eq!(parsed.code, "invalid_members");
}

#[tokio::test]
async fn group_key_share_route_deduplicates_member_pubkeys() {
    let state = test_state();
    let self_pubkey_hex = state.node.identity().public_key_hex();
    let app = build_router(state);
    let member_pubkey_hex = hex::encode(
        veil_crypto::signing::NostrSigner::from_secret([4u8; 32])
            .expect("valid secret")
            .public_key(),
    );
    let body = serde_json::to_string(&GroupKeyShareRequest {
        namespace: 32,
        channel_id: "group".to_string(),
        group_id: "group-alpha".to_string(),
        member_pubkeys: vec![
            self_pubkey_hex,
            member_pubkey_hex.clone(),
            member_pubkey_hex.to_ascii_uppercase(),
        ],
        rotate_key: true,
    })
    .unwrap();
    let response = app
        .oneshot(
            Request::builder()
                .uri("/group_key/share")
                .header("content-type", "application/json")
                .header("x-veil-token", "secret")
                .method("POST")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap_or_else(|_| Bytes::new());
    let parsed: GroupKeyShareResponse = serde_json::from_slice(&bytes).unwrap();
    assert!(parsed.queued);
    assert_eq!(parsed.shares, 1);
}
