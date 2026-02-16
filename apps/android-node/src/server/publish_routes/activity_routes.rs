use super::*;

pub(in crate::server) async fn publish_list(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<ListPublishRequest>,
) -> Response {
    if !authorized(&headers, &state.auth_token) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let identity = state.node.identity();
    let mut bundle = request.bundle;
    let pubkey_hex = identity.public_key_hex();
    if bundle.author_pubkey_hex.is_empty() {
        bundle.author_pubkey_hex = pubkey_hex.clone();
    } else if bundle.author_pubkey_hex != pubkey_hex {
        return bad_request("author_mismatch", "author pubkey does not match identity");
    }
    let feed_bundle = FeedBundle::List(bundle.clone());
    let payload = match serde_json::to_vec(&feed_bundle) {
        Ok(value) => value,
        Err(_) => return bad_request("invalid_bundle", "bundle serialization failed"),
    };
    let bundle_value = serde_json::to_value(&feed_bundle).unwrap_or_default();
    let message_id = state.node.enqueue_publish(PublishRequest {
        namespace: request.namespace,
        payload: String::from_utf8(payload).unwrap_or_default(),
    });
    state
        .node
        .inject_local_feed_bundle(bundle_value, blake3::hash(message_id.as_bytes()).into());
    Json(ListPublishResponse {
        message_id,
        queued: true,
        author_pubkey_hex: pubkey_hex,
    })
    .into_response()
}

pub(in crate::server) async fn publish_group_metadata(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<GroupMetadataPublishRequest>,
) -> Response {
    if !authorized(&headers, &state.auth_token) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let identity = state.node.identity();
    let mut bundle = request.bundle;
    let pubkey_hex = identity.public_key_hex();
    if bundle.author_pubkey_hex.is_empty() {
        bundle.author_pubkey_hex = pubkey_hex.clone();
    } else if bundle.author_pubkey_hex != pubkey_hex {
        return bad_request("author_mismatch", "author pubkey does not match identity");
    }
    let feed_bundle = FeedBundle::GroupMetadata(bundle.clone());
    let payload = match serde_json::to_vec(&feed_bundle) {
        Ok(value) => value,
        Err(_) => return bad_request("invalid_bundle", "bundle serialization failed"),
    };
    let bundle_value = serde_json::to_value(&feed_bundle).unwrap_or_default();
    let message_id = state.node.enqueue_publish(PublishRequest {
        namespace: request.namespace,
        payload: String::from_utf8(payload).unwrap_or_default(),
    });
    state
        .node
        .inject_local_feed_bundle(bundle_value, blake3::hash(message_id.as_bytes()).into());
    Json(GroupMetadataPublishResponse {
        message_id,
        queued: true,
        author_pubkey_hex: pubkey_hex,
    })
    .into_response()
}

pub(in crate::server) async fn publish_zap(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<ZapPublishRequest>,
) -> Response {
    if !authorized(&headers, &state.auth_token) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let identity = state.node.identity();
    let mut bundle = request.bundle;
    let pubkey_hex = identity.public_key_hex();
    if bundle.author_pubkey_hex.is_empty() {
        bundle.author_pubkey_hex = pubkey_hex.clone();
    } else if bundle.author_pubkey_hex != pubkey_hex {
        return bad_request("author_mismatch", "author pubkey does not match identity");
    }
    let feed_bundle = FeedBundle::Zap(bundle.clone());
    let payload = match serde_json::to_vec(&feed_bundle) {
        Ok(value) => value,
        Err(_) => return bad_request("invalid_bundle", "bundle serialization failed"),
    };
    let bundle_value = serde_json::to_value(&feed_bundle).unwrap_or_default();
    let message_id = state.node.enqueue_publish(PublishRequest {
        namespace: request.namespace,
        payload: String::from_utf8(payload).unwrap_or_default(),
    });
    state
        .node
        .inject_local_feed_bundle(bundle_value, blake3::hash(message_id.as_bytes()).into());
    Json(ZapPublishResponse {
        message_id,
        queued: true,
        author_pubkey_hex: pubkey_hex,
    })
    .into_response()
}

pub(in crate::server) async fn publish_app_preferences(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<AppPreferencesPublishRequest>,
) -> Response {
    if !authorized(&headers, &state.auth_token) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let identity = state.node.identity();
    let mut bundle = request.bundle;
    let pubkey_hex = identity.public_key_hex();
    if bundle.author_pubkey_hex.is_empty() {
        bundle.author_pubkey_hex = pubkey_hex.clone();
    } else if bundle.author_pubkey_hex != pubkey_hex {
        return bad_request("author_mismatch", "author pubkey does not match identity");
    }
    let feed_bundle = FeedBundle::AppPreferences(bundle.clone());
    let payload = match serde_json::to_vec(&feed_bundle) {
        Ok(value) => value,
        Err(_) => return bad_request("invalid_bundle", "bundle serialization failed"),
    };
    let bundle_value = serde_json::to_value(&feed_bundle).unwrap_or_default();
    let message_id = state.node.enqueue_publish(PublishRequest {
        namespace: request.namespace,
        payload: String::from_utf8(payload).unwrap_or_default(),
    });
    state
        .node
        .inject_local_feed_bundle(bundle_value, blake3::hash(message_id.as_bytes()).into());
    Json(AppPreferencesPublishResponse {
        message_id,
        queued: true,
        author_pubkey_hex: pubkey_hex,
    })
    .into_response()
}

pub(in crate::server) async fn publish_deletion(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<DeletionPublishRequest>,
) -> Response {
    if !authorized(&headers, &state.auth_token) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let identity = state.node.identity();
    let mut bundle = request.bundle;
    let pubkey_hex = identity.public_key_hex();
    if bundle.author_pubkey_hex.is_empty() {
        bundle.author_pubkey_hex = pubkey_hex.clone();
    } else if bundle.author_pubkey_hex != pubkey_hex {
        return bad_request("author_mismatch", "author pubkey does not match identity");
    }
    let feed_bundle = FeedBundle::Deletion(bundle.clone());
    let payload = match serde_json::to_vec(&feed_bundle) {
        Ok(value) => value,
        Err(_) => return bad_request("invalid_bundle", "bundle serialization failed"),
    };
    let bundle_value = serde_json::to_value(&feed_bundle).unwrap_or_default();
    let message_id = state.node.enqueue_publish(PublishRequest {
        namespace: request.namespace,
        payload: String::from_utf8(payload).unwrap_or_default(),
    });
    state
        .node
        .inject_local_feed_bundle(bundle_value, blake3::hash(message_id.as_bytes()).into());
    Json(DeletionPublishResponse {
        message_id,
        queued: true,
        author_pubkey_hex: pubkey_hex,
    })
    .into_response()
}

pub(in crate::server) async fn publish_repost(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<RepostPublishRequest>,
) -> Response {
    if !authorized(&headers, &state.auth_token) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let identity = state.node.identity();
    let mut bundle = request.bundle;
    let pubkey_hex = identity.public_key_hex();
    if bundle.author_pubkey_hex.is_empty() {
        bundle.author_pubkey_hex = pubkey_hex.clone();
    } else if bundle.author_pubkey_hex != pubkey_hex {
        return bad_request("author_mismatch", "author pubkey does not match identity");
    }
    let feed_bundle = FeedBundle::Repost(bundle.clone());
    let payload = match serde_json::to_vec(&feed_bundle) {
        Ok(value) => value,
        Err(_) => return bad_request("invalid_bundle", "bundle serialization failed"),
    };
    let bundle_value = serde_json::to_value(&feed_bundle).unwrap_or_default();
    let message_id = state.node.enqueue_publish(PublishRequest {
        namespace: request.namespace,
        payload: String::from_utf8(payload).unwrap_or_default(),
    });
    state
        .node
        .inject_local_feed_bundle(bundle_value, blake3::hash(message_id.as_bytes()).into());
    Json(RepostPublishResponse {
        message_id,
        queued: true,
        author_pubkey_hex: pubkey_hex,
    })
    .into_response()
}

pub(in crate::server) async fn publish_poll(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<PollPublishRequest>,
) -> Response {
    if !authorized(&headers, &state.auth_token) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let identity = state.node.identity();
    let mut bundle = request.bundle;
    let pubkey_hex = identity.public_key_hex();
    if bundle.author_pubkey_hex.is_empty() {
        bundle.author_pubkey_hex = pubkey_hex.clone();
    } else if bundle.author_pubkey_hex != pubkey_hex {
        return bad_request("author_mismatch", "author pubkey does not match identity");
    }
    let feed_bundle = FeedBundle::Poll(bundle.clone());
    let payload = match serde_json::to_vec(&feed_bundle) {
        Ok(value) => value,
        Err(_) => return bad_request("invalid_bundle", "bundle serialization failed"),
    };
    let bundle_value = serde_json::to_value(&feed_bundle).unwrap_or_default();
    let message_id = state.node.enqueue_publish(PublishRequest {
        namespace: request.namespace,
        payload: String::from_utf8(payload).unwrap_or_default(),
    });
    state
        .node
        .inject_local_feed_bundle(bundle_value, blake3::hash(message_id.as_bytes()).into());
    Json(PollPublishResponse {
        message_id,
        queued: true,
        author_pubkey_hex: pubkey_hex,
    })
    .into_response()
}

pub(in crate::server) async fn publish_poll_vote(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<PollVotePublishRequest>,
) -> Response {
    if !authorized(&headers, &state.auth_token) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let identity = state.node.identity();
    let mut bundle = request.bundle;
    let pubkey_hex = identity.public_key_hex();
    if bundle.author_pubkey_hex.is_empty() {
        bundle.author_pubkey_hex = pubkey_hex.clone();
    } else if bundle.author_pubkey_hex != pubkey_hex {
        return bad_request("author_mismatch", "author pubkey does not match identity");
    }
    let feed_bundle = FeedBundle::PollVote(bundle.clone());
    let payload = match serde_json::to_vec(&feed_bundle) {
        Ok(value) => value,
        Err(_) => return bad_request("invalid_bundle", "bundle serialization failed"),
    };
    let bundle_value = serde_json::to_value(&feed_bundle).unwrap_or_default();
    let message_id = state.node.enqueue_publish(PublishRequest {
        namespace: request.namespace,
        payload: String::from_utf8(payload).unwrap_or_default(),
    });
    state
        .node
        .inject_local_feed_bundle(bundle_value, blake3::hash(message_id.as_bytes()).into());
    Json(PollVotePublishResponse {
        message_id,
        queued: true,
        author_pubkey_hex: pubkey_hex,
    })
    .into_response()
}

pub(in crate::server) async fn publish_live_status(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<LiveStatusPublishRequest>,
) -> Response {
    if !authorized(&headers, &state.auth_token) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let identity = state.node.identity();
    let mut bundle = request.bundle;
    let pubkey_hex = identity.public_key_hex();
    if bundle.author_pubkey_hex.is_empty() {
        bundle.author_pubkey_hex = pubkey_hex.clone();
    } else if bundle.author_pubkey_hex != pubkey_hex {
        return bad_request("author_mismatch", "author pubkey does not match identity");
    }
    let feed_bundle = FeedBundle::LiveStatus(bundle.clone());
    let payload = match serde_json::to_vec(&feed_bundle) {
        Ok(value) => value,
        Err(_) => return bad_request("invalid_bundle", "bundle serialization failed"),
    };
    let bundle_value = serde_json::to_value(&feed_bundle).unwrap_or_default();
    let message_id = state.node.enqueue_publish(PublishRequest {
        namespace: request.namespace,
        payload: String::from_utf8(payload).unwrap_or_default(),
    });
    state
        .node
        .inject_local_feed_bundle(bundle_value, blake3::hash(message_id.as_bytes()).into());
    Json(LiveStatusPublishResponse {
        message_id,
        queued: true,
        author_pubkey_hex: pubkey_hex,
    })
    .into_response()
}
