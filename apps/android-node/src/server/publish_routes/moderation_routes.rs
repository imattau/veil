use super::*;

pub(in crate::server) async fn publish_follow(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<FollowPublishRequest>,
) -> Response {
    if !authorized(&headers, &state.auth_token) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let identity = state.node.identity();
    let mut bundle = request.bundle;
    let pubkey_hex = identity.public_key_hex();
    if bundle.follower_pubkey_hex.is_empty() {
        bundle.follower_pubkey_hex = pubkey_hex.clone();
    } else if bundle.follower_pubkey_hex != pubkey_hex {
        return bad_request(
            "follower_mismatch",
            "follower pubkey does not match identity",
        );
    }
    if !valid_channel(&bundle.channel_id) {
        return bad_request("invalid_channel", "channel_id is invalid");
    }
    if !valid_pubkey_hex(&bundle.followee_pubkey_hex) {
        return bad_request("invalid_followee", "followee pubkey invalid");
    }
    let feed_bundle = FeedBundle::Follow(bundle.clone());
    let payload = match serialize_feed_bundle_payload(&feed_bundle, Some(MAX_BUNDLE_JSON_BYTES)) {
        Ok(value) => value,
        Err(response) => return response,
    };
    let message_id =
        enqueue_and_inject_feed_bundle(&state, request.namespace, payload, &feed_bundle);
    state
        .node
        .trust_pubkey(hex_to_pubkey(&bundle.followee_pubkey_hex));
    state
        .protocol
        .update_wot_policy(state.node.wot_policy())
        .await;
    Json(FollowPublishResponse {
        message_id,
        queued: true,
        follower_pubkey_hex: pubkey_hex,
    })
    .into_response()
}

pub(in crate::server) async fn publish_mute(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<MutePublishRequest>,
) -> Response {
    if !authorized(&headers, &state.auth_token) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let identity = state.node.identity();
    let mut bundle = request.bundle;
    let pubkey_hex = identity.public_key_hex();
    if bundle.muter_pubkey_hex.is_empty() {
        bundle.muter_pubkey_hex = pubkey_hex.clone();
    } else if bundle.muter_pubkey_hex != pubkey_hex {
        return bad_request("muter_mismatch", "muter pubkey does not match identity");
    }
    if !valid_channel(&bundle.channel_id) {
        return bad_request("invalid_channel", "channel_id is invalid");
    }
    if !valid_pubkey_hex(&bundle.muted_pubkey_hex) {
        return bad_request("invalid_muted", "muted pubkey invalid");
    }
    if let Some(reason) = &bundle.reason {
        if reason.len() > MAX_REASON_LEN {
            return bad_request("reason_too_long", "reason too long");
        }
    }
    let feed_bundle = FeedBundle::Mute(bundle.clone());
    let payload = match serialize_feed_bundle_payload(&feed_bundle, Some(MAX_BUNDLE_JSON_BYTES)) {
        Ok(value) => value,
        Err(response) => return response,
    };
    let message_id = state.node.enqueue_publish(PublishRequest {
        namespace: request.namespace,
        payload,
    });
    state
        .node
        .mute_pubkey(hex_to_pubkey(&bundle.muted_pubkey_hex));
    state
        .protocol
        .update_wot_policy(state.node.wot_policy())
        .await;
    Json(MutePublishResponse {
        message_id,
        queued: true,
        muter_pubkey_hex: pubkey_hex,
    })
    .into_response()
}

pub(in crate::server) async fn publish_block(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<BlockPublishRequest>,
) -> Response {
    if !authorized(&headers, &state.auth_token) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let identity = state.node.identity();
    let mut bundle = request.bundle;
    let pubkey_hex = identity.public_key_hex();
    if bundle.blocker_pubkey_hex.is_empty() {
        bundle.blocker_pubkey_hex = pubkey_hex.clone();
    } else if bundle.blocker_pubkey_hex != pubkey_hex {
        return bad_request("blocker_mismatch", "blocker pubkey does not match identity");
    }
    if !valid_channel(&bundle.channel_id) {
        return bad_request("invalid_channel", "channel_id is invalid");
    }
    if !valid_pubkey_hex(&bundle.blocked_pubkey_hex) {
        return bad_request("invalid_blocked", "blocked pubkey invalid");
    }
    if let Some(reason) = &bundle.reason {
        if reason.len() > MAX_REASON_LEN {
            return bad_request("reason_too_long", "reason too long");
        }
    }
    let feed_bundle = FeedBundle::Block(bundle.clone());
    let payload = match serialize_feed_bundle_payload(&feed_bundle, Some(MAX_BUNDLE_JSON_BYTES)) {
        Ok(value) => value,
        Err(response) => return response,
    };
    let message_id = state.node.enqueue_publish(PublishRequest {
        namespace: request.namespace,
        payload,
    });
    state
        .node
        .block_pubkey(hex_to_pubkey(&bundle.blocked_pubkey_hex));
    state
        .protocol
        .update_wot_policy(state.node.wot_policy())
        .await;
    Json(BlockPublishResponse {
        message_id,
        queued: true,
        blocker_pubkey_hex: pubkey_hex,
    })
    .into_response()
}
