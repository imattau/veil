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
    let payload = match serialize_feed_bundle_payload(&feed_bundle, None) {
        Ok(value) => value,
        Err(response) => return response,
    };
    let message_id =
        enqueue_and_inject_feed_bundle(&state, request.namespace, payload, &feed_bundle);
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
    let payload = match serialize_feed_bundle_payload(&feed_bundle, None) {
        Ok(value) => value,
        Err(response) => return response,
    };
    let message_id =
        enqueue_and_inject_feed_bundle(&state, request.namespace, payload, &feed_bundle);
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
    let payload = match serialize_feed_bundle_payload(&feed_bundle, None) {
        Ok(value) => value,
        Err(response) => return response,
    };
    let message_id =
        enqueue_and_inject_feed_bundle(&state, request.namespace, payload, &feed_bundle);
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
    let payload = match serialize_feed_bundle_payload(&feed_bundle, None) {
        Ok(value) => value,
        Err(response) => return response,
    };
    let message_id =
        enqueue_and_inject_feed_bundle(&state, request.namespace, payload, &feed_bundle);
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
    let payload = match serialize_feed_bundle_payload(&feed_bundle, None) {
        Ok(value) => value,
        Err(response) => return response,
    };
    let message_id =
        enqueue_and_inject_feed_bundle(&state, request.namespace, payload, &feed_bundle);
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
    let payload = match serialize_feed_bundle_payload(&feed_bundle, None) {
        Ok(value) => value,
        Err(response) => return response,
    };
    let message_id =
        enqueue_and_inject_feed_bundle(&state, request.namespace, payload, &feed_bundle);
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
    let payload = match serialize_feed_bundle_payload(&feed_bundle, None) {
        Ok(value) => value,
        Err(response) => return response,
    };
    let message_id =
        enqueue_and_inject_feed_bundle(&state, request.namespace, payload, &feed_bundle);
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
    let payload = match serialize_feed_bundle_payload(&feed_bundle, None) {
        Ok(value) => value,
        Err(response) => return response,
    };
    let message_id =
        enqueue_and_inject_feed_bundle(&state, request.namespace, payload, &feed_bundle);
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
    let payload = match serialize_feed_bundle_payload(&feed_bundle, None) {
        Ok(value) => value,
        Err(response) => return response,
    };
    let message_id =
        enqueue_and_inject_feed_bundle(&state, request.namespace, payload, &feed_bundle);
    Json(LiveStatusPublishResponse {
        message_id,
        queued: true,
        author_pubkey_hex: pubkey_hex,
    })
    .into_response()
}
