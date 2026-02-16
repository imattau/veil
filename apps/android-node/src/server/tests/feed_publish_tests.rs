use super::*;

#[tokio::test]
async fn identity_returns_public_key() {
    let app = build_router(test_state());
    let response = app
        .oneshot(
            Request::builder()
                .uri("/identity")
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
    let parsed: IdentityResponse = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(parsed.public_key_hex.len(), 64);
}

#[tokio::test]
async fn identity_rotate_changes_pubkey() {
    let app = build_router(test_state());
    let first = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/identity")
                .header("x-veil-token", "secret")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let first_bytes = axum::body::to_bytes(first.into_body(), usize::MAX)
        .await
        .unwrap_or_else(|_| Bytes::new());
    let first_parsed: IdentityResponse = serde_json::from_slice(&first_bytes).unwrap();

    let rotated = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/identity/rotate")
                .method("POST")
                .header("x-veil-token", "secret")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(rotated.status(), StatusCode::OK);
    let rotated_bytes = axum::body::to_bytes(rotated.into_body(), usize::MAX)
        .await
        .unwrap_or_else(|_| Bytes::new());
    let rotated_parsed: IdentityRotateResponse = serde_json::from_slice(&rotated_bytes).unwrap();
    assert!(rotated_parsed.rotated);
    assert_ne!(first_parsed.public_key_hex, rotated_parsed.public_key_hex);
}

#[tokio::test]
async fn profile_publish_rejects_mismatched_author() {
    let app = build_router(test_state());
    let bundle = ProfileBundle {
        meta: BundleMeta {
            version: 1,
            created_at: 1_700_000_000,
        },
        channel_id: "general".to_string(),
        author_pubkey_hex: "11".repeat(32),
        display_name: "Test".to_string(),
        bio: "bio".to_string(),
        avatar_media_root: None,
    };
    let body = serde_json::to_string(&ProfilePublishRequest {
        namespace: 32,
        bundle,
    })
    .unwrap();
    let response = app
        .oneshot(
            Request::builder()
                .uri("/profile")
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
async fn profile_publish_accepts_empty_author() {
    let app = build_router(test_state());
    let bundle = ProfileBundle {
        meta: BundleMeta {
            version: 1,
            created_at: 1_700_000_001,
        },
        channel_id: "general".to_string(),
        author_pubkey_hex: String::new(),
        display_name: "Test".to_string(),
        bio: "bio".to_string(),
        avatar_media_root: None,
    };
    let body = serde_json::to_string(&ProfilePublishRequest {
        namespace: 32,
        bundle,
    })
    .unwrap();
    let response = app
        .oneshot(
            Request::builder()
                .uri("/profile")
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
    let parsed: ProfilePublishResponse = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(parsed.author_pubkey_hex.len(), 64);
}

#[tokio::test]
async fn post_publish_accepts_empty_author() {
    let app = build_router(test_state());
    let bundle = PostBundle {
        meta: BundleMeta {
            version: 1,
            created_at: 1_700_000_010,
        },
        channel_id: "general".to_string(),
        author_pubkey_hex: String::new(),
        text: "Hello".to_string(),
        media_roots: vec![],
        reply_to_root: None,
    };
    let body = serde_json::to_string(&PostPublishRequest {
        namespace: 32,
        bundle,
    })
    .unwrap();
    let response = app
        .oneshot(
            Request::builder()
                .uri("/post")
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
    let parsed: PostPublishResponse = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(parsed.author_pubkey_hex.len(), 64);
}

#[tokio::test]
async fn post_publish_queues_feed_bundle_envelope() {
    let state = test_state();
    let app = build_router(state.clone());
    let bundle = PostBundle {
        meta: BundleMeta {
            version: 1,
            created_at: 1_700_000_010,
        },
        channel_id: "general".to_string(),
        author_pubkey_hex: String::new(),
        text: "wrapped payload".to_string(),
        media_roots: vec![],
        reply_to_root: None,
    };
    let body = serde_json::to_string(&PostPublishRequest {
        namespace: 32,
        bundle,
    })
    .unwrap();
    let response = app
        .oneshot(
            Request::builder()
                .uri("/post")
                .method("POST")
                .header("content-type", "application/json")
                .header("x-veil-token", "secret")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let item = state
        .node
        .take_next_queued(now_ms)
        .expect("queued post payload");
    let payload: serde_json::Value =
        serde_json::from_str(&item.payload).expect("queued payload should be json");
    assert_eq!(payload.get("kind").and_then(|v| v.as_str()), Some("post"));
}

#[tokio::test]
async fn reaction_publish_accepts_empty_author() {
    let app = build_router(test_state());
    let body = serde_json::to_string(&ReactionPublishRequest {
        namespace: 32,
        bundle: ReactionBundle {
            meta: BundleMeta {
                version: 1,
                created_at: 1_700_000_030,
            },
            channel_id: "general".to_string(),
            author_pubkey_hex: String::new(),
            target_root: [0x11; 32],
            action_code: "like".to_string(),
        },
    })
    .unwrap();
    let response = app
        .oneshot(
            Request::builder()
                .uri("/reaction")
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
    let parsed: ReactionPublishResponse = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(parsed.author_pubkey_hex.len(), 64);
}

#[tokio::test]
async fn reaction_publish_rejects_empty_action() {
    let app = build_router(test_state());
    let body = serde_json::to_string(&ReactionPublishRequest {
        namespace: 32,
        bundle: ReactionBundle {
            meta: BundleMeta {
                version: 1,
                created_at: 1_700_000_031,
            },
            channel_id: "general".to_string(),
            author_pubkey_hex: String::new(),
            target_root: [0x11; 32],
            action_code: " ".to_string(),
        },
    })
    .unwrap();
    let response = app
        .oneshot(
            Request::builder()
                .uri("/reaction")
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
