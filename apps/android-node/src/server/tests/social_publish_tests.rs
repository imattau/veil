use super::*;

#[tokio::test]
async fn media_publish_accepts_empty_author() {
    let app = build_router(test_state());
    let bundle = MediaBundle {
        meta: BundleMeta {
            version: 1,
            created_at: 1_700_000_011,
        },
        channel_id: "general".to_string(),
        author_pubkey_hex: String::new(),
        mime_type: "image/png".to_string(),
        url: "https://example.com/a.png".to_string(),
        bytes_hint: 1024,
    };
    let body = serde_json::to_string(&MediaPublishRequest {
        namespace: 32,
        bundle,
    })
    .unwrap();
    let response = app
        .oneshot(
            Request::builder()
                .uri("/media")
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
    let parsed: MediaPublishResponse = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(parsed.author_pubkey_hex.len(), 64);
}

#[tokio::test]
async fn follow_publish_accepts_empty_follower() {
    let app = build_router(test_state());
    let bundle = FollowBundle {
        meta: BundleMeta {
            version: 1,
            created_at: 1_700_000_012,
        },
        channel_id: "general".to_string(),
        follower_pubkey_hex: String::new(),
        followee_pubkey_hex: "ff".repeat(32),
        at_step: 7,
    };
    let body = serde_json::to_string(&FollowPublishRequest {
        namespace: 32,
        bundle,
    })
    .unwrap();
    let response = app
        .oneshot(
            Request::builder()
                .uri("/follow")
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
    let parsed: FollowPublishResponse = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(parsed.follower_pubkey_hex.len(), 64);
}

#[tokio::test]
async fn follow_publish_rejects_invalid_followee() {
    let app = build_router(test_state());
    let bundle = FollowBundle {
        meta: BundleMeta {
            version: 1,
            created_at: 1_700_000_020,
        },
        channel_id: "general".to_string(),
        follower_pubkey_hex: String::new(),
        followee_pubkey_hex: "zz".to_string(),
        at_step: 7,
    };
    let body = serde_json::to_string(&FollowPublishRequest {
        namespace: 32,
        bundle,
    })
    .unwrap();
    let response = app
        .oneshot(
            Request::builder()
                .uri("/follow")
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
async fn mute_publish_accepts_empty_muter() {
    let app = build_router(test_state());
    let bundle = MuteBundle {
        meta: BundleMeta {
            version: 1,
            created_at: 1_700_000_013,
        },
        channel_id: "general".to_string(),
        muter_pubkey_hex: String::new(),
        muted_pubkey_hex: "aa".repeat(32),
        reason: Some("spam".to_string()),
        at_step: 9,
    };
    let body = serde_json::to_string(&MutePublishRequest {
        namespace: 32,
        bundle,
    })
    .unwrap();
    let response = app
        .oneshot(
            Request::builder()
                .uri("/mute")
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
    let parsed: MutePublishResponse = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(parsed.muter_pubkey_hex.len(), 64);
}

#[tokio::test]
async fn block_publish_accepts_empty_blocker() {
    let app = build_router(test_state());
    let bundle = BlockBundle {
        meta: BundleMeta {
            version: 1,
            created_at: 1_700_000_014,
        },
        channel_id: "general".to_string(),
        blocker_pubkey_hex: String::new(),
        blocked_pubkey_hex: "bb".repeat(32),
        reason: None,
        at_step: 11,
    };
    let body = serde_json::to_string(&BlockPublishRequest {
        namespace: 32,
        bundle,
    })
    .unwrap();
    let response = app
        .oneshot(
            Request::builder()
                .uri("/block")
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
    let parsed: BlockPublishResponse = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(parsed.blocker_pubkey_hex.len(), 64);
}

#[tokio::test]
async fn post_publish_triggers_loopback() {
    let state = test_state();
    let mut rx = state.node.subscribe_events();
    let app = build_router(state);

    let bundle = PostBundle {
        meta: BundleMeta {
            version: 1,
            created_at: 1_700_000_010,
        },
        channel_id: "general".to_string(),
        author_pubkey_hex: String::new(),
        text: "Loopback test".to_string(),
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

    // The node emits "publish_queued" first, then we call inject_local_feed_bundle
    let first = rx.try_recv().expect("first event should be emitted");
    assert_eq!(first.event, "publish_queued");

    // Now we should get the feed_bundle event
    let second = rx.try_recv().expect("second event should be emitted");
    assert_eq!(second.event, "feed_bundle");
    assert_eq!(second.data["text"], "Loopback test");
}
