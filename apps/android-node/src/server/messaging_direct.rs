use super::ingest_routes::queue_raw_object_payload;
use super::*;

pub(super) async fn publish_direct_message(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<DirectMessagePublishRequest>,
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
    if !valid_pubkey_hex(&bundle.recipient_pubkey_hex) {
        return bad_request("invalid_recipient", "recipient pubkey invalid");
    }
    let feed_bundle = FeedBundle::DirectMessage(bundle.clone());
    let payload = match serialize_feed_bundle_payload(&feed_bundle, Some(MAX_BUNDLE_JSON_BYTES)) {
        Ok(value) => value,
        Err(response) => return response,
    };
    let message_id =
        enqueue_and_inject_feed_bundle(&state, request.namespace, payload, &feed_bundle);
    Json(DirectMessagePublishResponse {
        message_id,
        queued: true,
        author_pubkey_hex: pubkey_hex,
    })
    .into_response()
}

pub(super) async fn publish_direct_message_text(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<DirectMessageTextPublishRequest>,
) -> Response {
    if !authorized(&headers, &state.auth_token) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    if !valid_channel(&request.channel_id) {
        return bad_request("invalid_channel", "channel_id is invalid");
    }
    if !valid_pubkey_hex(&request.recipient_pubkey_hex) {
        return bad_request("invalid_recipient", "recipient pubkey invalid");
    }
    if request.text.trim().is_empty() {
        return bad_request("invalid_text", "text is empty");
    }
    if request.text.len() > MAX_TEXT_LEN {
        return bad_request("text_too_long", "text too long");
    }
    let identity = state.node.identity();
    let pubkey_hex = identity.public_key_hex();
    let encrypted_payload = match encrypt_direct_message_payload(
        identity.secret_key,
        &pubkey_hex,
        &request.recipient_pubkey_hex,
        request.text.as_bytes(),
    ) {
        Ok(value) => value,
        Err(err) => return bad_request("encrypt_failed", &err),
    };
    let (_object_msg, object_root) =
        match queue_raw_object_payload(&state, request.namespace, &encrypted_payload).await {
            Ok(value) => value,
            Err(err) => return (StatusCode::INTERNAL_SERVER_ERROR, err).into_response(),
        };
    let bundle = DirectMessageBundle {
        meta: BundleMeta {
            version: 1,
            created_at: current_unix_seconds(),
        },
        channel_id: request.channel_id,
        author_pubkey_hex: pubkey_hex.clone(),
        recipient_pubkey_hex: request.recipient_pubkey_hex,
        ciphertext_root: object_root,
        reply_to_root: request.reply_to_root,
    };
    let feed_bundle = FeedBundle::DirectMessage(bundle);
    let payload = match serialize_feed_bundle_payload(&feed_bundle, Some(MAX_BUNDLE_JSON_BYTES)) {
        Ok(value) => value,
        Err(response) => return response,
    };
    let message_id =
        enqueue_and_inject_feed_bundle(&state, request.namespace, payload, &feed_bundle);
    Json(DirectMessagePublishResponse {
        message_id,
        queued: true,
        author_pubkey_hex: pubkey_hex,
    })
    .into_response()
}
