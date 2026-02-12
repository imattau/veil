use std::net::SocketAddr;

use axum::{
    extract::{Query, State, WebSocketUpgrade},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use base64::Engine;
use tracing::info;

use crate::api::{
    AppPreferencesPublishRequest, AppPreferencesPublishResponse, BlockPublishRequest,
    BlockPublishResponse, ContactImportRequest, ContactImportResponse, ContactListResponse,
    DeletionPublishRequest, DeletionPublishResponse, DirectMessagePublishRequest,
    DirectMessagePublishResponse, DiscoveryAnnounceRequest, DiscoveryGossipRequest,
    DiscoveryGossipResponse, DiscoveryLookupRequest, DiscoveryLookupResponse, ErrorResponse,
    FeedResponse, FollowPublishRequest, FollowPublishResponse, GroupMessagePublishRequest,
    GroupMessagePublishResponse, GroupMetadataPublishRequest, GroupMetadataPublishResponse,
    HealthResponse, IdentityExportResponse, IdentityImportRequest, IdentityResponse,
    IdentityRotateResponse, ListPublishRequest, ListPublishResponse, LiveStatusPublishRequest,
    LiveStatusPublishResponse, MediaPublishRequest, MediaPublishResponse, MutePublishRequest,
    MutePublishResponse, ObjectFetchResponse, ObjectPublishRequest, ObjectPublishResponse,
    PolicyConfigRequest, PolicyConfigResponse, PolicyListsResponse, PolicySetRequest,
    PolicySetResponse, PolicySummaryResponse, PollPublishRequest, PollPublishResponse,
    PollVotePublishRequest, PollVotePublishResponse, PostPublishRequest, PostPublishResponse,
    ProfilePublishRequest, ProfilePublishResponse, PublishRequest, PublishResponse,
    ReactionPublishRequest, ReactionPublishResponse, RepostPublishRequest, RepostPublishResponse,
    ShardFetchResponse, StatusResponse, SubscribeRequest, SubscribeResponse,
    SubscriptionListResponse, UnsubscribeRequest, UnsubscribeResponse, ZapPublishRequest,
    ZapPublishResponse,
};
use crate::discovery::{
    build_self_contact, handle_discovery_announce, handle_discovery_gossip,
    handle_discovery_lookup, DiscoveryMessage,
};
use crate::state::NodeState;
use crate::ProtocolEngine;
use veil_schema_feed::FeedBundle;

#[derive(Clone)]
pub struct AppState {
    pub node: NodeState,
    pub protocol: std::sync::Arc<ProtocolEngine>,
    pub auth_token: String,
    pub version: String,
}

pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/status", get(status))
        .route("/feed", get(feed))
        .route("/subscriptions", get(subscriptions))
        .route("/identity", get(identity))
        .route("/identity/rotate", post(rotate_identity))
        .route("/identity/export", get(export_identity))
        .route("/identity/import", post(import_identity))
        .route("/publish", post(publish))
        .route("/publish_object", post(publish_object))
        .route("/profile", post(publish_profile))
        .route("/post", post(publish_post))
        .route("/reaction", post(publish_reaction))
        .route("/direct_message", post(publish_direct_message))
        .route("/group_message", post(publish_group_message))
        .route("/media", post(publish_media))
        .route("/follow", post(publish_follow))
        .route("/mute", post(publish_mute))
        .route("/block", post(publish_block))
        .route("/list", post(publish_list))
        .route("/group_metadata", post(publish_group_metadata))
        .route("/zap", post(publish_zap))
        .route("/app_preferences", post(publish_app_preferences))
        .route("/deletion", post(publish_deletion))
        .route("/repost", post(publish_repost))
        .route("/poll", post(publish_poll))
        .route("/poll_vote", post(publish_poll_vote))
        .route("/live_status", post(publish_live_status))
        .route("/subscribe", post(subscribe))
        .route("/unsubscribe", post(unsubscribe))
        .route("/policy", get(policy_summary))
        .route("/policy/lists", get(policy_lists))
        .route("/policy/explain", post(policy_explain))
        .route("/policy/config", post(update_policy_config))
        .route("/policy/trust", post(policy_trust))
        .route("/policy/untrust", post(policy_untrust))
        .route("/policy/mute", post(policy_mute))
        .route("/policy/unmute", post(policy_unmute))
        .route("/policy/block", post(policy_block))
        .route("/policy/unblock", post(policy_unblock))
        .route("/contact", get(contact_list))
        .route("/contact", post(contact_import))
        .route("/contact/self", get(contact_self))
        .route("/discovery/announce", post(discovery_announce))
        .route("/discovery/lookup", post(discovery_lookup))
        .route("/discovery/gossip", post(discovery_gossip))
        .route("/shard/:id", get(fetch_shard))
        .route("/object/:root", get(fetch_object))
        .route("/events", get(events_ws))
        .with_state(state)
}

pub async fn serve(addr: SocketAddr, state: AppState) {
    let app = build_router(state);
    info!(%addr, "android node listening");
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("bind address");
    axum::serve(listener, app).await.expect("serve");
}

const MAX_RAW_PAYLOAD_BYTES: usize = 256 * 1024;
const MAX_BUNDLE_JSON_BYTES: usize = 256 * 1024;
const MAX_CHANNEL_LEN: usize = 64;
const MAX_NAME_LEN: usize = 64;
const MAX_TEXT_LEN: usize = 4096;
const MAX_BIO_LEN: usize = 1024;
const MAX_URL_LEN: usize = 1024;
const MAX_MIME_LEN: usize = 128;
const MAX_REASON_LEN: usize = 256;
const MAX_ACTION_LEN: usize = 32;
const MAX_GROUP_ID_LEN: usize = 64;

async fn health(State(state): State<AppState>) -> Response {
    Json(HealthResponse {
        status: "ok".to_string(),
        version: state.version.clone(),
    })
    .into_response()
}

async fn status(State(state): State<AppState>, headers: HeaderMap) -> Response {
    if !authorized(&headers, &state.auth_token) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let payload: StatusResponse = state.node.status();
    Json(payload).into_response()
}

async fn feed(State(state): State<AppState>, headers: HeaderMap) -> Response {
    if !authorized(&headers, &state.auth_token) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let events = state.node.get_feed(50);
    Json(FeedResponse { events }).into_response()
}

async fn subscriptions(State(state): State<AppState>, headers: HeaderMap) -> Response {
    if !authorized(&headers, &state.auth_token) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let subscriptions = state.node.get_subscriptions();
    Json(SubscriptionListResponse { subscriptions }).into_response()
}

async fn publish(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<PublishRequest>,
) -> Response {
    if !authorized(&headers, &state.auth_token) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    if request.payload.as_bytes().len() > MAX_RAW_PAYLOAD_BYTES {
        return bad_request("payload_too_large", "payload exceeds max size");
    }
    let message_id = state.node.enqueue_publish(request);
    let response = PublishResponse {
        message_id,
        queued: true,
    };
    Json(response).into_response()
}

async fn publish_object(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<ObjectPublishRequest>,
) -> Response {
    if !authorized(&headers, &state.auth_token) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let payload = match base64::engine::general_purpose::STANDARD.decode(&request.payload_b64) {
        Ok(bytes) => bytes,
        Err(_) => return bad_request("invalid_base64", "payload must be base64 encoded"),
    };
    if payload.len() > MAX_RAW_PAYLOAD_BYTES {
        return bad_request("payload_too_large", "payload exceeds max size");
    }
    let object_root = blake3::hash(&payload);
    let wrapped_payload = serde_json::json!({
        "kind": "raw_b64",
        "payload_b64": request.payload_b64,
    })
    .to_string();
    let message_id = state.node.enqueue_publish(PublishRequest {
        namespace: request.namespace,
        payload: wrapped_payload,
    });
    Json(ObjectPublishResponse {
        object_root: hex::encode(object_root.as_bytes()),
        message_id,
        queued: true,
    })
    .into_response()
}

async fn identity(State(state): State<AppState>, headers: HeaderMap) -> Response {
    if !authorized(&headers, &state.auth_token) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let identity = state.node.identity();
    Json(IdentityResponse {
        public_key_hex: identity.public_key_hex(),
    })
    .into_response()
}

async fn rotate_identity(State(state): State<AppState>, headers: HeaderMap) -> Response {
    if !authorized(&headers, &state.auth_token) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let identity = state.node.rotate_identity();
    state
        .protocol
        .update_identity(identity.public_key, identity.signer())
        .await;
    Json(IdentityRotateResponse {
        public_key_hex: identity.public_key_hex(),
        rotated: true,
    })
    .into_response()
}

async fn export_identity(State(state): State<AppState>, headers: HeaderMap) -> Response {
    if !authorized(&headers, &state.auth_token) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let (public_key_hex, secret_key_hex) = state.node.export_identity();
    Json(IdentityExportResponse {
        public_key_hex,
        secret_key_hex,
    })
    .into_response()
}

async fn import_identity(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<IdentityImportRequest>,
) -> Response {
    if !authorized(&headers, &state.auth_token) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    match state.node.import_identity(request.secret_key_hex) {
        Ok(identity) => {
            state
                .protocol
                .update_identity(identity.public_key, identity.signer())
                .await;
            Json(IdentityResponse {
                public_key_hex: identity.public_key_hex(),
            })
            .into_response()
        }
        Err(err) => bad_request("import_failed", &err),
    }
}

async fn publish_profile(
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
    Json(ProfilePublishResponse {
        message_id,
        queued: true,
        author_pubkey_hex: pubkey_hex,
    })
    .into_response()
}

async fn publish_post(
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
    Json(PostPublishResponse {
        message_id,
        queued: true,
        author_pubkey_hex: pubkey_hex,
    })
    .into_response()
}

async fn publish_reaction(
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
    Json(ReactionPublishResponse {
        message_id,
        queued: true,
        author_pubkey_hex: pubkey_hex,
    })
    .into_response()
}

async fn publish_direct_message(
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

async fn publish_group_message(
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

async fn publish_media(
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
    Json(MediaPublishResponse {
        message_id,
        queued: true,
        author_pubkey_hex: pubkey_hex,
    })
    .into_response()
}

async fn publish_follow(
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

async fn publish_mute(
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
    let payload = match serde_json::to_vec(&feed_bundle) {
        Ok(value) => value,
        Err(_) => return bad_request("invalid_bundle", "bundle serialization failed"),
    };
    if payload.len() > MAX_BUNDLE_JSON_BYTES {
        return bad_request("bundle_too_large", "bundle exceeds max size");
    }
    let message_id = state.node.enqueue_publish(PublishRequest {
        namespace: request.namespace,
        payload: String::from_utf8(payload).unwrap_or_default(),
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

async fn publish_block(
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
    let payload = match serde_json::to_vec(&feed_bundle) {
        Ok(value) => value,
        Err(_) => return bad_request("invalid_bundle", "bundle serialization failed"),
    };
    if payload.len() > MAX_BUNDLE_JSON_BYTES {
        return bad_request("bundle_too_large", "bundle exceeds max size");
    }
    let message_id = state.node.enqueue_publish(PublishRequest {
        namespace: request.namespace,
        payload: String::from_utf8(payload).unwrap_or_default(),
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

async fn publish_list(
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

async fn publish_group_metadata(
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

async fn publish_zap(
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

async fn publish_app_preferences(
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

async fn publish_deletion(
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

async fn publish_repost(
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

async fn publish_poll(
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

async fn publish_poll_vote(
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

async fn publish_live_status(
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

async fn policy_summary(State(state): State<AppState>, headers: HeaderMap) -> Response {
    if !authorized(&headers, &state.auth_token) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let summary = state.node.policy_summary();
    Json(PolicySummaryResponse {
        trusted: summary.trusted,
        muted: summary.muted,
        blocked: summary.blocked,
        endorsements: summary.endorsements,
        config: summary.config,
    })
    .into_response()
}

async fn policy_lists(State(state): State<AppState>, headers: HeaderMap) -> Response {
    if !authorized(&headers, &state.auth_token) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let lists = state.node.policy_lists();
    Json(PolicyListsResponse {
        trusted_pubkeys: lists.trusted_pubkeys,
        muted_pubkeys: lists.muted_pubkeys,
        blocked_pubkeys: lists.blocked_pubkeys,
    })
    .into_response()
}

async fn policy_explain(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<PolicySetRequest>,
) -> Response {
    if !authorized(&headers, &state.auth_token) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    if !valid_pubkey_hex(&request.pubkey_hex) {
        return bad_request("invalid_pubkey", "pubkey invalid");
    }
    let pubkey = hex_to_pubkey(&request.pubkey_hex);
    let policy = state.node.wot_policy();
    let explanation = policy.explain_publisher(pubkey, 0);
    Json(explanation).into_response()
}

async fn update_policy_config(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<PolicyConfigRequest>,
) -> Response {
    if !authorized(&headers, &state.auth_token) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    state.node.update_policy_config(request.config);
    state
        .protocol
        .update_wot_policy(state.node.wot_policy())
        .await;
    Json(PolicyConfigResponse { updated: true }).into_response()
}

async fn policy_trust(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<PolicySetRequest>,
) -> Response {
    if !authorized(&headers, &state.auth_token) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    if !valid_pubkey_hex(&request.pubkey_hex) {
        return bad_request("invalid_pubkey", "pubkey invalid");
    }
    state.node.trust_pubkey(hex_to_pubkey(&request.pubkey_hex));
    state
        .protocol
        .update_wot_policy(state.node.wot_policy())
        .await;
    Json(PolicySetResponse { applied: true }).into_response()
}

async fn policy_untrust(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<PolicySetRequest>,
) -> Response {
    if !authorized(&headers, &state.auth_token) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    if !valid_pubkey_hex(&request.pubkey_hex) {
        return bad_request("invalid_pubkey", "pubkey invalid");
    }
    state
        .node
        .untrust_pubkey(hex_to_pubkey(&request.pubkey_hex));
    state
        .protocol
        .update_wot_policy(state.node.wot_policy())
        .await;
    Json(PolicySetResponse { applied: true }).into_response()
}

async fn policy_mute(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<PolicySetRequest>,
) -> Response {
    if !authorized(&headers, &state.auth_token) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    if !valid_pubkey_hex(&request.pubkey_hex) {
        return bad_request("invalid_pubkey", "pubkey invalid");
    }
    state.node.mute_pubkey(hex_to_pubkey(&request.pubkey_hex));
    state
        .protocol
        .update_wot_policy(state.node.wot_policy())
        .await;
    Json(PolicySetResponse { applied: true }).into_response()
}

async fn policy_unmute(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<PolicySetRequest>,
) -> Response {
    if !authorized(&headers, &state.auth_token) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    if !valid_pubkey_hex(&request.pubkey_hex) {
        return bad_request("invalid_pubkey", "pubkey invalid");
    }
    state.node.unmute_pubkey(hex_to_pubkey(&request.pubkey_hex));
    state
        .protocol
        .update_wot_policy(state.node.wot_policy())
        .await;
    Json(PolicySetResponse { applied: true }).into_response()
}

async fn policy_block(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<PolicySetRequest>,
) -> Response {
    if !authorized(&headers, &state.auth_token) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    if !valid_pubkey_hex(&request.pubkey_hex) {
        return bad_request("invalid_pubkey", "pubkey invalid");
    }
    state.node.block_pubkey(hex_to_pubkey(&request.pubkey_hex));
    state
        .protocol
        .update_wot_policy(state.node.wot_policy())
        .await;
    Json(PolicySetResponse { applied: true }).into_response()
}

async fn policy_unblock(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<PolicySetRequest>,
) -> Response {
    if !authorized(&headers, &state.auth_token) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    if !valid_pubkey_hex(&request.pubkey_hex) {
        return bad_request("invalid_pubkey", "pubkey invalid");
    }
    state
        .node
        .unblock_pubkey(hex_to_pubkey(&request.pubkey_hex));
    state
        .protocol
        .update_wot_policy(state.node.wot_policy())
        .await;
    Json(PolicySetResponse { applied: true }).into_response()
}

async fn contact_list(State(state): State<AppState>, headers: HeaderMap) -> Response {
    if !authorized(&headers, &state.auth_token) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let contacts = state.node.contacts();
    Json(ContactListResponse { contacts }).into_response()
}

async fn contact_self(State(state): State<AppState>, headers: HeaderMap) -> Response {
    if !authorized(&headers, &state.auth_token) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let contact = build_self_contact(&state.node, &state.protocol);
    Json(contact).into_response()
}

async fn contact_import(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<ContactImportRequest>,
) -> Response {
    if !authorized(&headers, &state.auth_token) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    if request.contact.pubkey_hex.len() != 64 {
        return bad_request("invalid_pubkey", "pubkey invalid");
    }
    state.node.add_contact(request.contact.clone());
    state.protocol.add_contact(&request.contact).await;
    Json(ContactImportResponse { imported: true }).into_response()
}

async fn discovery_announce(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<DiscoveryAnnounceRequest>,
) -> Response {
    if !authorized(&headers, &state.auth_token) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    if request.contact.pubkey_hex.len() != 64 {
        return bad_request("invalid_pubkey", "pubkey invalid");
    }
    let contact = request.contact.clone();
    let response = handle_discovery_announce(&state.node, request, 16);
    for contact in &response.neighbors {
        state.protocol.add_contact(contact).await;
    }
    let _ = state
        .protocol
        .publish_discovery(DiscoveryMessage::announce(contact))
        .await;
    Json(response).into_response()
}

async fn discovery_lookup(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<DiscoveryLookupRequest>,
) -> Response {
    if !authorized(&headers, &state.auth_token) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let response: DiscoveryLookupResponse = handle_discovery_lookup(&state.node, request.clone());
    let reply_to = state.protocol.peer_id();
    let msg = DiscoveryMessage::lookup(request.peer_id, request.pubkey_hex, reply_to);
    let _ = state.protocol.publish_discovery(msg).await;
    Json(response).into_response()
}

async fn discovery_gossip(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<DiscoveryGossipRequest>,
) -> Response {
    if !authorized(&headers, &state.auth_token) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    for contact in &request.contacts {
        if contact.pubkey_hex.len() == 64 {
            state.protocol.add_contact(contact).await;
        }
    }
    let response: DiscoveryGossipResponse =
        handle_discovery_gossip(&state.node, request.clone(), 24);
    let msg = DiscoveryMessage::gossip(request.contacts);
    let _ = state.protocol.publish_discovery(msg).await;
    Json(response).into_response()
}

async fn fetch_shard(
    State(state): State<AppState>,
    headers: HeaderMap,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Response {
    if !authorized(&headers, &state.auth_token) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    if !valid_pubkey_hex(&id) {
        return bad_request("invalid_shard_id", "shard id invalid");
    }
    let mut shard_id = [0u8; 32];
    if let Ok(bytes) = hex::decode(&id) {
        if bytes.len() == 32 {
            shard_id.copy_from_slice(&bytes);
        }
    }
    let shard = state.protocol.get_cached_shard(shard_id).await;
    match shard {
        Some(bytes) => Json(ShardFetchResponse {
            shard_b64: base64::engine::general_purpose::STANDARD.encode(bytes),
        })
        .into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn fetch_object(
    State(state): State<AppState>,
    headers: HeaderMap,
    axum::extract::Path(root): axum::extract::Path<String>,
) -> Response {
    if !authorized(&headers, &state.auth_token) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    if !valid_pubkey_hex(&root) {
        return bad_request("invalid_object_root", "object root invalid");
    }
    let mut object_root = [0u8; 32];
    if let Ok(bytes) = hex::decode(&root) {
        if bytes.len() == 32 {
            object_root.copy_from_slice(&bytes);
        }
    }
    let object = state.protocol.reconstruct_payload(object_root).await;
    match object {
        Some(bytes) => Json(ObjectFetchResponse {
            object_b64: base64::engine::general_purpose::STANDARD.encode(bytes),
        })
        .into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn subscribe(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<SubscribeRequest>,
) -> Response {
    if !authorized(&headers, &state.auth_token) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let changed = state.node.subscribe(&request.tag);
    Json(SubscribeResponse {
        subscribed: changed,
    })
    .into_response()
}

async fn unsubscribe(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<UnsubscribeRequest>,
) -> Response {
    if !authorized(&headers, &state.auth_token) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let changed = state.node.unsubscribe(&request.tag);
    Json(UnsubscribeResponse {
        unsubscribed: changed,
    })
    .into_response()
}

async fn events_ws(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<EventsQuery>,
    ws: WebSocketUpgrade,
) -> Response {
    if !authorized(&headers, &state.auth_token) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    ws.on_upgrade(move |mut socket| async move {
        let (backlog, mut rx) = state.node.subscribe_events_since(query.since);
        for event in backlog {
            if socket
                .send(axum::extract::ws::Message::Text(
                    serde_json::to_string(&event).unwrap_or_default(),
                ))
                .await
                .is_err()
            {
                return;
            }
        }
        let status_event = state.node.emit_status_event();
        let _ = socket
            .send(axum::extract::ws::Message::Text(
                serde_json::to_string(&status_event).unwrap_or_default(),
            ))
            .await;
        while let Ok(event) = rx.recv().await {
            let payload = match serde_json::to_string(&event) {
                Ok(value) => value,
                Err(_) => continue,
            };
            if socket
                .send(axum::extract::ws::Message::Text(payload))
                .await
                .is_err()
            {
                break;
            }
        }
    })
}

#[derive(Debug, serde::Deserialize)]
struct EventsQuery {
    since: Option<u64>,
}

fn authorized(headers: &HeaderMap, token: &str) -> bool {
    if token.is_empty() {
        return true;
    }
    headers
        .get("x-veil-token")
        .and_then(|value| value.to_str().ok())
        .map(|value| value == token)
        .unwrap_or(false)
}

fn bad_request(code: &str, message: &str) -> Response {
    let payload = ErrorResponse {
        code: code.to_string(),
        message: message.to_string(),
    };
    (StatusCode::BAD_REQUEST, Json(payload)).into_response()
}

fn valid_pubkey_hex(value: &str) -> bool {
    value.len() == 64 && value.chars().all(|c| c.is_ascii_hexdigit())
}

fn valid_channel(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= MAX_CHANNEL_LEN
}

fn hex_to_pubkey(value: &str) -> [u8; 32] {
    let mut out = [0u8; 32];
    if let Ok(bytes) = hex::decode(value) {
        if bytes.len() == 32 {
            out.copy_from_slice(&bytes);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::ContactBundle;
    use crate::api::DiscoveryAnnounceResponse;
    use axum::body::{Body, Bytes};
    use http::{Request, StatusCode};
    use tower::ServiceExt;
    use veil_schema_feed::{
        BlockBundle, BundleMeta, DirectMessageBundle, FollowBundle, GroupMessageBundle,
        MediaBundle, MuteBundle, PostBundle, ProfileBundle, ReactionBundle,
    };

    fn test_state() -> AppState {
        let node = NodeState::new("0.1-test");
        let identity = node.identity();
        let protocol_config = crate::default_protocol_config(
            "ws://127.0.0.1:9001/ws".to_string(),
            "test-node".to_string(),
            32,
            identity.public_key,
            identity.signer(),
        );
        let protocol =
            std::sync::Arc::new(ProtocolEngine::new(protocol_config).expect("protocol init"));
        AppState {
            node,
            protocol,
            auth_token: "secret".to_string(),
            version: "0.1-test".to_string(),
        }
    }

    #[tokio::test]
    async fn rejects_missing_token() {
        let app = build_router(test_state());
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/status")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn health_allows_unauthenticated_access() {
        let app = build_router(test_state());
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap_or_else(|_| Bytes::new());
        let parsed: HealthResponse = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(parsed.status, "ok");
        assert!(!parsed.version.is_empty());
    }

    #[tokio::test]
    async fn publish_rejects_oversized_payload() {
        let app = build_router(test_state());
        let body = serde_json::to_string(&PublishRequest {
            namespace: 32,
            payload: "a".repeat(MAX_RAW_PAYLOAD_BYTES + 1),
        })
        .unwrap();
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/publish")
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
    async fn returns_status_with_token() {
        let app = build_router(test_state());
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/status")
                    .header("x-veil-token", "secret")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn publish_queues_message() {
        let app = build_router(test_state());
        let body = serde_json::to_string(&PublishRequest {
            namespace: 32,
            payload: "hello".to_string(),
        })
        .unwrap();
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/publish")
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

    #[tokio::test]
    async fn status_reflects_publish_queue() {
        let app = build_router(test_state());
        let body = serde_json::to_string(&PublishRequest {
            namespace: 32,
            payload: "hello".to_string(),
        })
        .unwrap();
        let _ = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/publish")
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
                    .uri("/status")
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
        let parsed: StatusResponse = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(parsed.queue.pending, 1);
    }

    #[tokio::test]
    async fn publish_response_contains_message_id() {
        let app = build_router(test_state());
        let body = serde_json::to_string(&PublishRequest {
            namespace: 32,
            payload: "hello".to_string(),
        })
        .unwrap();
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/publish")
                    .method("POST")
                    .header("content-type", "application/json")
                    .header("x-veil-token", "secret")
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap_or_else(|_| Bytes::new());
        let parsed: PublishResponse = serde_json::from_slice(&bytes).unwrap();
        assert!(!parsed.message_id.is_nil());
    }

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
        let mut config = veil_node::policy::WotConfig::default();
        config.trusted_forward_quota = 0.55;
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

    #[tokio::test]
    async fn contact_import_adds_contact() {
        let app = build_router(test_state());
        let contact = ContactBundle {
            peer_id: "peer-a".to_string(),
            ws_url: Some("ws://127.0.0.1:9001/ws".to_string()),
            quic_addr: Some("127.0.0.1:9000".to_string()),
            pubkey_hex: "aa".repeat(32),
            rpc_url: Some("http://127.0.0.1:7788".to_string()),
            lan_addrs: vec!["192.168.1.5:9000".to_string()],
        };
        let body = serde_json::to_string(&ContactImportRequest { contact }).unwrap();
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/contact")
                    .method("POST")
                    .header("content-type", "application/json")
                    .header("x-veil-token", "secret")
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/contact")
                    .header("x-veil-token", "secret")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap_or_else(|_| Bytes::new());
        let parsed: ContactListResponse = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(parsed.contacts.len(), 1);
    }

    #[tokio::test]
    async fn discovery_announce_returns_neighbors() {
        let app = build_router(test_state());
        let contact = ContactBundle {
            peer_id: "peer-b".to_string(),
            ws_url: None,
            quic_addr: Some("127.0.0.1:9002".to_string()),
            pubkey_hex: "bb".repeat(32),
            rpc_url: None,
            lan_addrs: Vec::new(),
        };
        let body = serde_json::to_string(&DiscoveryAnnounceRequest { contact }).unwrap();
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/discovery/announce")
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
        let parsed: DiscoveryAnnounceResponse = serde_json::from_slice(&bytes).unwrap();
        assert!(parsed.accepted);
    }

    #[tokio::test]
    async fn discovery_lookup_returns_contacts() {
        let app = build_router(test_state());
        let contact = ContactBundle {
            peer_id: "peer-c".to_string(),
            ws_url: None,
            quic_addr: Some("127.0.0.1:9003".to_string()),
            pubkey_hex: "cc".repeat(32),
            rpc_url: None,
            lan_addrs: Vec::new(),
        };
        let announce = serde_json::to_string(&DiscoveryAnnounceRequest { contact }).unwrap();
        let _ = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/discovery/announce")
                    .method("POST")
                    .header("content-type", "application/json")
                    .header("x-veil-token", "secret")
                    .body(Body::from(announce))
                    .unwrap(),
            )
            .await
            .unwrap();
        let lookup = serde_json::to_string(&DiscoveryLookupRequest {
            peer_id: Some("peer-c".to_string()),
            pubkey_hex: None,
            limit: Some(4),
        })
        .unwrap();
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/discovery/lookup")
                    .method("POST")
                    .header("content-type", "application/json")
                    .header("x-veil-token", "secret")
                    .body(Body::from(lookup))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap_or_else(|_| Bytes::new());
        let parsed: DiscoveryLookupResponse = serde_json::from_slice(&bytes).unwrap();
        assert!(!parsed.contacts.is_empty());
    }

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
        let rotated_parsed: IdentityRotateResponse =
            serde_json::from_slice(&rotated_bytes).unwrap();
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
    async fn group_message_publish_accepts_empty_author() {
        let app = build_router(test_state());
        let body = serde_json::to_string(&GroupMessagePublishRequest {
            namespace: 32,
            bundle: GroupMessageBundle {
                meta: BundleMeta {
                    version: 1,
                    created_at: 1_700_000_034,
                },
                channel_id: "general".to_string(),
                author_pubkey_hex: String::new(),
                group_id: "group-alpha".to_string(),
                ciphertext_root: [0x33; 32],
                reply_to_root: None,
            },
        })
        .unwrap();
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/group_message")
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
        let parsed: GroupMessagePublishResponse = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(parsed.author_pubkey_hex.len(), 64);
    }

    #[tokio::test]
    async fn group_message_publish_rejects_empty_group_id() {
        let app = build_router(test_state());
        let body = serde_json::to_string(&GroupMessagePublishRequest {
            namespace: 32,
            bundle: GroupMessageBundle {
                meta: BundleMeta {
                    version: 1,
                    created_at: 1_700_000_035,
                },
                channel_id: "general".to_string(),
                author_pubkey_hex: String::new(),
                group_id: " ".to_string(),
                ciphertext_root: [0x33; 32],
                reply_to_root: None,
            },
        })
        .unwrap();
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/group_message")
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
}
