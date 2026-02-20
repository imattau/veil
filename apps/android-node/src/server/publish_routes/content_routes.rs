use super::*;

pub(in crate::server) async fn publish_profile(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<ProfilePublishRequest>,
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
    if !valid_channel(&bundle.channel_id) {
        return bad_request("invalid_channel", "channel_id is invalid");
    }
    if bundle.display_name.len() > MAX_NAME_LEN {
        return bad_request("display_name_too_long", "display_name too long");
    }
    if bundle.bio.len() > MAX_BIO_LEN {
        return bad_request("bio_too_long", "bio too long");
    }
    let feed_bundle = FeedBundle::Profile(bundle.clone());
    let payload = match serialize_feed_bundle_payload(&feed_bundle, Some(MAX_BUNDLE_JSON_BYTES)) {
        Ok(value) => value,
        Err(response) => return response,
    };
    let message_id =
        enqueue_and_inject_feed_bundle(&state, request.namespace, payload, &feed_bundle);
    Json(ProfilePublishResponse {
        message_id,
        queued: true,
        author_pubkey_hex: pubkey_hex,
    })
    .into_response()
}

pub(in crate::server) async fn publish_post(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<PostPublishRequest>,
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
    if !valid_channel(&bundle.channel_id) {
        return bad_request("invalid_channel", "channel_id is invalid");
    }
    if bundle.text.len() > MAX_TEXT_LEN {
        return bad_request("text_too_long", "text too long");
    }
    let feed_bundle = FeedBundle::Post(bundle.clone());
    let payload = match serialize_feed_bundle_payload(&feed_bundle, Some(MAX_BUNDLE_JSON_BYTES)) {
        Ok(value) => value,
        Err(response) => return response,
    };
    let message_id =
        enqueue_and_inject_feed_bundle(&state, request.namespace, payload, &feed_bundle);
    Json(PostPublishResponse {
        message_id,
        queued: true,
        author_pubkey_hex: pubkey_hex,
    })
    .into_response()
}

pub(in crate::server) async fn publish_reaction(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<ReactionPublishRequest>,
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
    if !valid_channel(&bundle.channel_id) {
        return bad_request("invalid_channel", "channel_id is invalid");
    }
    if bundle.action_code.trim().is_empty() || bundle.action_code.len() > MAX_ACTION_LEN {
        return bad_request("invalid_action", "action_code invalid");
    }
    let feed_bundle = FeedBundle::Reaction(bundle.clone());
    let payload = match serialize_feed_bundle_payload(&feed_bundle, Some(MAX_BUNDLE_JSON_BYTES)) {
        Ok(value) => value,
        Err(response) => return response,
    };
    let message_id =
        enqueue_and_inject_feed_bundle(&state, request.namespace, payload, &feed_bundle);
    Json(ReactionPublishResponse {
        message_id,
        queued: true,
        author_pubkey_hex: pubkey_hex,
    })
    .into_response()
}

pub(in crate::server) async fn publish_media(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<MediaPublishRequest>,
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
    if !valid_channel(&bundle.channel_id) {
        return bad_request("invalid_channel", "channel_id is invalid");
    }
    if bundle.mime_type.len() > MAX_MIME_LEN {
        return bad_request("mime_too_long", "mime_type too long");
    }
    if bundle.url.len() > MAX_URL_LEN {
        return bad_request("url_too_long", "url too long");
    }
    let feed_bundle = FeedBundle::Media(bundle.clone());
    let payload = match serialize_feed_bundle_payload(&feed_bundle, Some(MAX_BUNDLE_JSON_BYTES)) {
        Ok(value) => value,
        Err(response) => return response,
    };
    let message_id =
        enqueue_and_inject_feed_bundle(&state, request.namespace, payload, &feed_bundle);
    Json(MediaPublishResponse {
        message_id,
        queued: true,
        author_pubkey_hex: pubkey_hex,
    })
    .into_response()
}
