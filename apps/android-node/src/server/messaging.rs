use super::ingest_routes::queue_raw_object_payload;
use super::*;
use std::collections::HashSet;

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
    let payload = match serde_json::to_vec(&feed_bundle) {
        Ok(value) => value,
        Err(_) => return bad_request("invalid_bundle", "bundle serialization failed"),
    };
    if payload.len() > MAX_BUNDLE_JSON_BYTES {
        return bad_request("bundle_too_large", "bundle exceeds max size");
    }
    let bundle_value = serde_json::to_value(&feed_bundle).unwrap_or_default();
    let message_id = state.node.enqueue_publish(PublishRequest {
        namespace: request.namespace,
        payload: String::from_utf8(payload).unwrap_or_default(),
    });
    state
        .node
        .inject_local_feed_bundle(bundle_value, blake3::hash(message_id.as_bytes()).into());
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
    let payload = match serde_json::to_vec(&feed_bundle) {
        Ok(value) => value,
        Err(_) => return bad_request("invalid_bundle", "bundle serialization failed"),
    };
    if payload.len() > MAX_BUNDLE_JSON_BYTES {
        return bad_request("bundle_too_large", "bundle exceeds max size");
    }
    let bundle_value = serde_json::to_value(&feed_bundle).unwrap_or_default();
    let message_id = state.node.enqueue_publish(PublishRequest {
        namespace: request.namespace,
        payload: String::from_utf8(payload).unwrap_or_default(),
    });
    state
        .node
        .inject_local_feed_bundle(bundle_value, blake3::hash(message_id.as_bytes()).into());
    Json(DirectMessagePublishResponse {
        message_id,
        queued: true,
        author_pubkey_hex: pubkey_hex,
    })
    .into_response()
}

pub(super) async fn publish_group_message(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<GroupMessagePublishRequest>,
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
    if bundle.group_id.trim().is_empty() || bundle.group_id.len() > MAX_GROUP_ID_LEN {
        return bad_request("invalid_group", "group_id invalid");
    }
    let feed_bundle = FeedBundle::GroupMessage(bundle.clone());
    let payload = match serde_json::to_vec(&feed_bundle) {
        Ok(value) => value,
        Err(_) => return bad_request("invalid_bundle", "bundle serialization failed"),
    };
    if payload.len() > MAX_BUNDLE_JSON_BYTES {
        return bad_request("bundle_too_large", "bundle exceeds max size");
    }
    let bundle_value = serde_json::to_value(&feed_bundle).unwrap_or_default();
    let message_id = state.node.enqueue_publish(PublishRequest {
        namespace: request.namespace,
        payload: String::from_utf8(payload).unwrap_or_default(),
    });
    state
        .node
        .inject_local_feed_bundle(bundle_value, blake3::hash(message_id.as_bytes()).into());
    Json(GroupMessagePublishResponse {
        message_id,
        queued: true,
        author_pubkey_hex: pubkey_hex,
    })
    .into_response()
}

pub(super) async fn publish_group_message_text(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<GroupMessageTextPublishRequest>,
) -> Response {
    if !authorized(&headers, &state.auth_token) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    if !valid_channel(&request.channel_id) {
        return bad_request("invalid_channel", "channel_id is invalid");
    }
    if request.group_id.trim().is_empty() || request.group_id.len() > MAX_GROUP_ID_LEN {
        return bad_request("invalid_group", "group_id invalid");
    }
    if request.text.trim().is_empty() {
        return bad_request("invalid_text", "text is empty");
    }
    if request.text.len() > MAX_TEXT_LEN {
        return bad_request("text_too_long", "text too long");
    }
    let identity = state.node.identity();
    let pubkey_hex = identity.public_key_hex();
    for member in &request.member_pubkeys {
        if !valid_pubkey_hex(member) {
            return bad_request("invalid_member", "member pubkey invalid");
        }
    }
    let share_recipients = unique_share_recipients(&request.member_pubkeys, &pubkey_hex);
    let (key_id, group_key) = state.node.ensure_group_key(&request.group_id);
    if !share_recipients.is_empty() {
        let shares = queue_group_key_shares(
            &state,
            request.namespace,
            &request.group_id,
            &pubkey_hex,
            identity.secret_key,
            &key_id,
            group_key,
            &share_recipients,
        )
        .await;
        if shares < share_recipients.len() {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    code: "key_share_enqueue_failed".to_string(),
                    message: "failed to queue one or more group key shares".to_string(),
                }),
            )
                .into_response();
        }
    }
    let encrypted_payload = match encrypt_group_message_payload(
        &request.group_id,
        &key_id,
        group_key,
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
    let bundle = GroupMessageBundle {
        meta: BundleMeta {
            version: 1,
            created_at: current_unix_seconds(),
        },
        channel_id: request.channel_id,
        author_pubkey_hex: pubkey_hex.clone(),
        group_id: request.group_id,
        ciphertext_root: object_root,
        reply_to_root: request.reply_to_root,
    };
    let feed_bundle = FeedBundle::GroupMessage(bundle);
    let payload = match serde_json::to_vec(&feed_bundle) {
        Ok(value) => value,
        Err(_) => return bad_request("invalid_bundle", "bundle serialization failed"),
    };
    if payload.len() > MAX_BUNDLE_JSON_BYTES {
        return bad_request("bundle_too_large", "bundle exceeds max size");
    }
    let bundle_value = serde_json::to_value(&feed_bundle).unwrap_or_default();
    let message_id = state.node.enqueue_publish(PublishRequest {
        namespace: request.namespace,
        payload: String::from_utf8(payload).unwrap_or_default(),
    });
    state
        .node
        .inject_local_feed_bundle(bundle_value, blake3::hash(message_id.as_bytes()).into());
    Json(GroupMessagePublishResponse {
        message_id,
        queued: true,
        author_pubkey_hex: pubkey_hex,
    })
    .into_response()
}

pub(super) async fn share_group_key(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<GroupKeyShareRequest>,
) -> Response {
    if !authorized(&headers, &state.auth_token) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    if !valid_channel(&request.channel_id) {
        return bad_request("invalid_channel", "channel_id is invalid");
    }
    if request.group_id.trim().is_empty() || request.group_id.len() > MAX_GROUP_ID_LEN {
        return bad_request("invalid_group", "group_id invalid");
    }
    if request.member_pubkeys.is_empty() {
        return bad_request("invalid_members", "member_pubkeys is empty");
    }
    for member in &request.member_pubkeys {
        if !valid_pubkey_hex(member) {
            return bad_request("invalid_member", "member pubkey invalid");
        }
    }
    let identity = state.node.identity();
    let pubkey_hex = identity.public_key_hex();
    let share_recipients = unique_share_recipients(&request.member_pubkeys, &pubkey_hex);
    if share_recipients.is_empty() {
        return bad_request("invalid_members", "member_pubkeys contains no recipients");
    }
    let (key_id, group_key) = if request.rotate_key {
        state.node.rotate_group_key(&request.group_id)
    } else {
        state.node.ensure_group_key(&request.group_id)
    };
    let shares = queue_group_key_shares(
        &state,
        request.namespace,
        &request.group_id,
        &pubkey_hex,
        identity.secret_key,
        &key_id,
        group_key,
        &share_recipients,
    )
    .await;
    Json(GroupKeyShareResponse {
        queued: shares == share_recipients.len(),
        key_id,
        shares,
    })
    .into_response()
}

fn unique_share_recipients(member_pubkeys: &[String], sender_pubkey_hex: &str) -> Vec<String> {
    let sender = sender_pubkey_hex.to_ascii_lowercase();
    let mut seen = HashSet::new();
    let mut recipients = Vec::new();
    for member in member_pubkeys {
        let normalized = member.to_ascii_lowercase();
        if normalized == sender {
            continue;
        }
        if seen.insert(normalized.clone()) {
            recipients.push(normalized);
        }
    }
    recipients
}

#[allow(clippy::too_many_arguments)]
async fn queue_group_key_shares(
    state: &AppState,
    namespace: u16,
    group_id: &str,
    sender_pubkey_hex: &str,
    sender_secret: [u8; 32],
    key_id: &str,
    group_key: [u8; 32],
    member_pubkeys: &[String],
) -> usize {
    let mut shares = 0usize;
    for member in member_pubkeys {
        if member == sender_pubkey_hex {
            continue;
        }
        let payload = match encrypt_group_key_share_payload(
            sender_secret,
            sender_pubkey_hex,
            member,
            group_id,
            key_id,
            group_key,
        ) {
            Ok(value) => value,
            Err(_) => continue,
        };
        if queue_raw_object_payload(state, namespace, &payload)
            .await
            .is_ok()
        {
            shares += 1;
        }
    }
    shares
}
