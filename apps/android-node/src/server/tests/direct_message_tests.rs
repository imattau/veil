use super::*;

#[tokio::test]
async fn direct_message_publish_accepts_empty_author() {
    let app = build_router(test_state());
    let body = serde_json::to_string(&DirectMessagePublishRequest {
        namespace: 32,
        bundle: DirectMessageBundle {
            meta: BundleMeta {
                version: 1,
                created_at: 1_700_000_032,
            },
            channel_id: "dm".to_string(),
            author_pubkey_hex: String::new(),
            recipient_pubkey_hex: "aa".repeat(32),
            ciphertext_root: [0x22; 32],
            reply_to_root: None,
        },
    })
    .unwrap();
    let response = app
        .oneshot(
            Request::builder()
                .uri("/direct_message")
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
    let parsed: DirectMessagePublishResponse = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(parsed.author_pubkey_hex.len(), 64);
}

#[tokio::test]
async fn direct_message_publish_rejects_invalid_recipient() {
    let app = build_router(test_state());
    let body = serde_json::to_string(&DirectMessagePublishRequest {
        namespace: 32,
        bundle: DirectMessageBundle {
            meta: BundleMeta {
                version: 1,
                created_at: 1_700_000_033,
            },
            channel_id: "dm".to_string(),
            author_pubkey_hex: String::new(),
            recipient_pubkey_hex: "zz".to_string(),
            ciphertext_root: [0x22; 32],
            reply_to_root: None,
        },
    })
    .unwrap();
    let response = app
        .oneshot(
            Request::builder()
                .uri("/direct_message")
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
async fn direct_message_text_publish_encrypts_and_queues() {
    let app = build_router(test_state());
    let recipient_pubkey_hex = hex::encode(
        veil_crypto::signing::NostrSigner::from_secret([3u8; 32])
            .expect("valid secret")
            .public_key(),
    );
    let body = serde_json::to_string(&DirectMessageTextPublishRequest {
        namespace: 32,
        channel_id: "dm".to_string(),
        recipient_pubkey_hex,
        text: "secret hello".to_string(),
        reply_to_root: None,
    })
    .unwrap();
    let response = app
        .oneshot(
            Request::builder()
                .uri("/direct_message_text")
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
