use super::*;

#[tokio::test]
async fn policy_summary_reflects_trust() {
    let app = build_router(test_state());
    let pubkey = "aa".repeat(32);
    let body = serde_json::to_string(&PolicySetRequest { pubkey_hex: pubkey }).unwrap();
    let _ = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/policy/trust")
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
                .uri("/policy")
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
    let parsed: PolicySummaryResponse = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(parsed.trusted, 1);
}

#[tokio::test]
async fn policy_lists_include_trusted_muted_and_blocked() {
    let app = build_router(test_state());
    let trusted = "aa".repeat(32);
    let muted = "bb".repeat(32);
    let blocked = "cc".repeat(32);

    for (route, pubkey_hex) in [
        ("/policy/trust", trusted.clone()),
        ("/policy/mute", muted.clone()),
        ("/policy/block", blocked.clone()),
    ] {
        let body = serde_json::to_string(&PolicySetRequest { pubkey_hex }).unwrap();
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(route)
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

    let response = app
        .oneshot(
            Request::builder()
                .uri("/policy/lists")
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
    let parsed: PolicyListsResponse = serde_json::from_slice(&bytes).unwrap();
    assert!(parsed.trusted_pubkeys.contains(&trusted));
    assert!(parsed.muted_pubkeys.contains(&muted));
    assert!(parsed.blocked_pubkeys.contains(&blocked));
}

#[tokio::test]
async fn policy_config_updates() {
    let app = build_router(test_state());
    let config = veil_node::policy::WotConfig {
        trusted_forward_quota: 0.55,
        ..Default::default()
    };
    let body = serde_json::to_string(&PolicyConfigRequest { config }).unwrap();
    let response = app
        .oneshot(
            Request::builder()
                .uri("/policy/config")
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
