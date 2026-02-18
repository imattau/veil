use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::Json;
use serde_json::json;

use crate::http_server::{admin_authenticated, VpsAppState};

pub(crate) async fn admin_policy_summary(
    State(state): State<VpsAppState>,
    headers: HeaderMap,
) -> impl IntoResponse {
    if !admin_authenticated(&headers, &state.admin_auth) {
        return (StatusCode::UNAUTHORIZED, "admin auth required").into_response();
    }
    let config = state
        .runtime_config
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let summary = config.wot_policy.summary();
    Json(summary).into_response()
}

pub(crate) async fn admin_policy_config_set(
    State(state): State<VpsAppState>,
    headers: HeaderMap,
    Json(payload): Json<veil_node::policy::WotConfig>,
) -> impl IntoResponse {
    if !admin_authenticated(&headers, &state.admin_auth) {
        return (StatusCode::UNAUTHORIZED, "admin auth required").into_response();
    }
    let mut config = state
        .runtime_config
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    config.wot_policy.update_config(payload);
    (StatusCode::OK, Json(json!({"ok": true}))).into_response()
}

pub(crate) async fn admin_policy_trust(
    State(state): State<VpsAppState>,
    headers: HeaderMap,
    Json(payload): Json<veil_android_node::PolicySetRequest>,
) -> impl IntoResponse {
    apply_policy_pubkey_mutation(&state, &headers, payload, |cfg, pubkey| {
        cfg.wot_policy.trust(pubkey)
    })
}

pub(crate) async fn admin_policy_untrust(
    State(state): State<VpsAppState>,
    headers: HeaderMap,
    Json(payload): Json<veil_android_node::PolicySetRequest>,
) -> impl IntoResponse {
    apply_policy_pubkey_mutation(&state, &headers, payload, |cfg, pubkey| {
        cfg.wot_policy.untrust(pubkey)
    })
}

pub(crate) async fn admin_policy_mute(
    State(state): State<VpsAppState>,
    headers: HeaderMap,
    Json(payload): Json<veil_android_node::PolicySetRequest>,
) -> impl IntoResponse {
    apply_policy_pubkey_mutation(&state, &headers, payload, |cfg, pubkey| {
        cfg.wot_policy.mute(pubkey)
    })
}

pub(crate) async fn admin_policy_unmute(
    State(state): State<VpsAppState>,
    headers: HeaderMap,
    Json(payload): Json<veil_android_node::PolicySetRequest>,
) -> impl IntoResponse {
    apply_policy_pubkey_mutation(&state, &headers, payload, |cfg, pubkey| {
        cfg.wot_policy.unmute(pubkey)
    })
}

pub(crate) async fn admin_policy_block(
    State(state): State<VpsAppState>,
    headers: HeaderMap,
    Json(payload): Json<veil_android_node::PolicySetRequest>,
) -> impl IntoResponse {
    apply_policy_pubkey_mutation(&state, &headers, payload, |cfg, pubkey| {
        cfg.wot_policy.block(pubkey)
    })
}

pub(crate) async fn admin_policy_unblock(
    State(state): State<VpsAppState>,
    headers: HeaderMap,
    Json(payload): Json<veil_android_node::PolicySetRequest>,
) -> impl IntoResponse {
    apply_policy_pubkey_mutation(&state, &headers, payload, |cfg, pubkey| {
        cfg.wot_policy.unblock(pubkey)
    })
}

fn apply_policy_pubkey_mutation(
    state: &VpsAppState,
    headers: &HeaderMap,
    payload: veil_android_node::PolicySetRequest,
    mutate: impl FnOnce(&mut veil_node::config::NodeRuntimeConfig, [u8; 32]),
) -> axum::response::Response {
    if !admin_authenticated(headers, &state.admin_auth) {
        return (StatusCode::UNAUTHORIZED, "admin auth required").into_response();
    }
    let Some(pubkey) = parse_pubkey(&payload.pubkey_hex) else {
        return (StatusCode::BAD_REQUEST, "invalid pubkey hex").into_response();
    };
    let mut config = state
        .runtime_config
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    mutate(&mut config, pubkey);
    (StatusCode::OK, Json(json!({"ok": true}))).into_response()
}

fn parse_pubkey(pubkey_hex: &str) -> Option<[u8; 32]> {
    let bytes = hex::decode(pubkey_hex).ok()?;
    <[u8; 32]>::try_from(bytes.as_slice()).ok()
}
