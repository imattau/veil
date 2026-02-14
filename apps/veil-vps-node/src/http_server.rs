use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use rand::RngCore;
use serde::Deserialize;
use serde_json::json;
use tower_http::cors::CorsLayer;

use crate::settings_db::SettingsStore;
use crate::{
    decode_nostr_secret_input, now_unix_secs, AdminAuthState, AdminLoginRequest,
    AdminSettingUpsertRequest, MetricsState,
};
use veil_crypto::signing::{NostrSigner, Signer};

#[derive(Clone)]
pub struct VpsAppState {
    pub metrics: Arc<MetricsState>,
    pub peer_snapshot: Arc<Mutex<Vec<String>>>,
    pub admin_auth: Arc<AdminAuthState>,
    pub shutdown: Arc<AtomicBool>,
}

pub fn build_router(state: VpsAppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/healthz", get(health))
        .route("/metrics", get(metrics))
        .route("/peers", get(peers))
        .route("/admin-api/login", post(admin_login))
        .route("/admin-api/logout", post(admin_logout))
        .route("/admin-api/restart", post(admin_restart))
        .route("/admin-api/status", get(admin_status))
        .route("/admin-api/identity", get(admin_identity))
        .route("/admin-api/metrics", get(admin_metrics))
        .route("/admin-api/peers", get(admin_peers))
        .route("/admin-api/settings", get(admin_settings_get))
        .route("/admin-api/settings", post(admin_settings_set))
        .route("/admin-api/settings", delete(admin_settings_delete))
        .layer(CorsLayer::permissive())
        .with_state(state)
}

fn admin_authenticated(headers: &HeaderMap, admin: &AdminAuthState) -> bool {
    let Some(auth) = headers.get("authorization").and_then(|v| v.to_str().ok()) else {
        return false;
    };
    let Some(token) = auth.strip_prefix("Bearer ") else {
        return false;
    };
    if token.is_empty() {
        return false;
    }
    let now = now_unix_secs();
    let mut sessions = admin.sessions.lock().unwrap_or_else(|e| e.into_inner());
    let expired_tokens: Vec<String> = sessions
        .iter()
        .filter_map(|(t, expires)| {
            if *expires <= now {
                Some(t.clone())
            } else {
                None
            }
        })
        .collect();
    sessions.retain(|_, expires| *expires > now);
    drop(sessions);
    if !expired_tokens.is_empty() {
        for t in expired_tokens {
            admin.persist_session_remove(&t);
        }
        admin.persist_expired_prune(now);
    }
    let sessions = admin.sessions.lock().unwrap_or_else(|e| e.into_inner());
    sessions.get(token).is_some_and(|expires| *expires > now)
}

async fn health() -> &'static str {
    "ok"
}

async fn metrics(State(state): State<VpsAppState>) -> impl IntoResponse {
    metrics_body(&state.metrics)
}

fn metrics_body(metrics: &MetricsState) -> String {
    format!(
        "veil_ticks_total {}\nveil_delivered_total {}\nveil_send_failures_total {}\nveil_ack_clears_total {}\nveil_fast_outbound_ok {}\nveil_fast_outbound_err {}\nveil_fallback_outbound_ok {}\nveil_fallback_outbound_err {}\nveil_fast_inbound {}\nveil_fallback_inbound {}\nveil_nostr_bridge_events_total {}\nveil_nostr_bridge_payload_bytes_total {}\nveil_nostr_bridge_enabled {}\nveil_nostr_bridge_relays_configured {}\n",
        metrics.ticks.load(Ordering::Relaxed),
        metrics.delivered.load(Ordering::Relaxed),
        metrics.send_failures.load(Ordering::Relaxed),
        metrics.ack_clears.load(Ordering::Relaxed),
        metrics.last_fast_outbound_ok.load(Ordering::Relaxed),
        metrics.last_fast_outbound_err.load(Ordering::Relaxed),
        metrics.last_fallback_outbound_ok.load(Ordering::Relaxed),
        metrics.last_fallback_outbound_err.load(Ordering::Relaxed),
        metrics.last_fast_inbound.load(Ordering::Relaxed),
        metrics.last_fallback_inbound.load(Ordering::Relaxed),
        metrics.nostr_bridge_events_total.load(Ordering::Relaxed),
        metrics.nostr_bridge_payload_bytes_total.load(Ordering::Relaxed),
        metrics.nostr_bridge_enabled.load(Ordering::Relaxed),
        metrics.nostr_bridge_relays_configured.load(Ordering::Relaxed),
    )
}

#[derive(Deserialize)]
struct PeersQuery {
    limit: Option<usize>,
    prefix: Option<String>,
}

async fn peers(
    State(state): State<VpsAppState>,
    Query(query): Query<PeersQuery>,
) -> impl IntoResponse {
    let limit = query.limit.unwrap_or(200).min(1000);
    let peers = state
        .peer_snapshot
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let iter = peers.iter().filter(|peer| {
        query
            .prefix
            .as_ref()
            .map(|p| peer.starts_with(p))
            .unwrap_or(true)
    });
    iter.take(limit).cloned().collect::<Vec<_>>().join("\n")
}

async fn admin_login(
    State(state): State<VpsAppState>,
    Json(payload): Json<AdminLoginRequest>,
) -> impl IntoResponse {
    tracing::info!("admin login: attempt received");
    let input = payload.secret.trim();
    tracing::info!("admin login: input prefix: {}", if input.len() > 5 { &input[..5] } else { "***" });
    
    let Some(secret) = decode_nostr_secret_input(&payload.secret) else {
        tracing::warn!("admin login: failed to decode secret input (tried hex and nsec)");
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"ok": false, "error": "secret must be hex or nsec"})),
        );
    };
    let Ok(signer) = NostrSigner::from_secret(secret) else {
        tracing::warn!("admin login: invalid nostr secret");
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"ok": false, "error": "invalid nostr secret"})),
        );
    };
    let login_pubkey = signer.public_key();
    if login_pubkey != state.admin_auth.server_pubkey {
        tracing::warn!(
            "admin login: wrong identity key. expected={}, provided={}",
            state.admin_auth.server_pubkey_hex,
            hex::encode(login_pubkey)
        );
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({"ok": false, "error": "wrong identity key"})),
        );
    }
    let mut raw = [0_u8; 32];
    rand::thread_rng().fill_bytes(&mut raw);
    let token = hex::encode(raw);
    let expires = now_unix_secs() + state.admin_auth.session_ttl_secs;
    state.admin_auth.add_session(token.clone(), expires);
    tracing::info!("admin login: successful for {}", state.admin_auth.server_pubkey_hex);
    (
        StatusCode::OK,
        Json(json!({
            "ok": true,
            "token": token,
            "server_pubkey": state.admin_auth.server_pubkey_hex,
            "expires_at": expires
        })),
    )
}

async fn admin_logout(State(state): State<VpsAppState>, headers: HeaderMap) -> impl IntoResponse {
    let Some(auth) = headers.get("authorization").and_then(|v| v.to_str().ok()) else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({"ok": false, "error": "admin auth required"})),
        );
    };
    let Some(token) = auth.strip_prefix("Bearer ") else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({"ok": false, "error": "admin auth required"})),
        );
    };
    let _ = state.admin_auth.revoke_session(token);
    (
        StatusCode::OK,
        Json(json!({"ok": true, "logged_out": true})),
    )
}

async fn admin_restart(State(state): State<VpsAppState>, headers: HeaderMap) -> impl IntoResponse {
    if !admin_authenticated(&headers, &state.admin_auth) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({"ok": false, "error": "admin auth required"})),
        );
    }
    state.shutdown.store(true, Ordering::Relaxed);
    (
        StatusCode::ACCEPTED,
        Json(json!({"ok": true, "restarting": true})),
    )
}

async fn admin_status(
    State(state): State<VpsAppState>,
    headers: HeaderMap,
) -> Json<serde_json::Value> {
    let is_auth = admin_authenticated(&headers, &state.admin_auth);
    Json(json!({
        "ok": is_auth,
        "server_pubkey": state.admin_auth.server_pubkey_hex
    }))
}

async fn admin_identity(State(state): State<VpsAppState>, headers: HeaderMap) -> impl IntoResponse {
    if !admin_authenticated(&headers, &state.admin_auth) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({"ok": false, "error": "admin auth required"})),
        );
    }
    (
        StatusCode::OK,
        Json(json!({
            "ok": true,
            "server_pubkey": state.admin_auth.server_pubkey_hex,
            "server_secret_hex": state.admin_auth.server_secret_hex,
            "server_secret_nsec": state.admin_auth.server_secret_nsec
        })),
    )
}

async fn admin_metrics(State(state): State<VpsAppState>, headers: HeaderMap) -> impl IntoResponse {
    if !admin_authenticated(&headers, &state.admin_auth) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({"ok": false, "error": "admin auth required"})).into_response(),
        );
    }
    (StatusCode::OK, metrics_body(&state.metrics).into_response())
}

async fn admin_peers(
    State(state): State<VpsAppState>,
    headers: HeaderMap,
    Query(query): Query<PeersQuery>,
) -> impl IntoResponse {
    if !admin_authenticated(&headers, &state.admin_auth) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({"ok": false, "error": "admin auth required"})).into_response(),
        );
    }
    let limit = query.limit.unwrap_or(200).min(1000);
    let peers = state
        .peer_snapshot
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let iter = peers.iter().filter(|peer| {
        query
            .prefix
            .as_ref()
            .map(|p| peer.starts_with(p))
            .unwrap_or(true)
    });
    let body = iter.take(limit).cloned().collect::<Vec<_>>().join("\n");
    (StatusCode::OK, body.into_response())
}

#[derive(Deserialize)]
struct SettingsKeyQuery {
    key: Option<String>,
}

async fn admin_settings_get(
    State(state): State<VpsAppState>,
    headers: HeaderMap,
    Query(query): Query<SettingsKeyQuery>,
) -> impl IntoResponse {
    if !admin_authenticated(&headers, &state.admin_auth) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({"ok": false, "error": "admin auth required"})),
        );
    }
    let store = match SettingsStore::open(&state.admin_auth.settings_db_path) {
        Ok(store) => store,
        Err(err) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"ok": false, "error": err})),
            )
        }
    };
    if let Some(key) = query.key {
        match store.get(&key) {
            Some(value) => (
                StatusCode::OK,
                Json(json!({"ok": true, "key": key, "value": value})),
            ),
            None => (
                StatusCode::NOT_FOUND,
                Json(json!({"ok": false, "error": "setting not found"})),
            ),
        }
    } else {
        match store.list() {
            Ok(items) => (StatusCode::OK, Json(json!({"ok": true, "items": items}))),
            Err(err) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"ok": false, "error": err})),
            ),
        }
    }
}

async fn admin_settings_set(
    State(state): State<VpsAppState>,
    headers: HeaderMap,
    Json(payload): Json<AdminSettingUpsertRequest>,
) -> impl IntoResponse {
    if !admin_authenticated(&headers, &state.admin_auth) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({"ok": false, "error": "admin auth required"})),
        );
    }
    let key = payload.key.trim().to_string();
    if key.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"ok": false, "error": "key is required"})),
        );
    }
    match SettingsStore::open(&state.admin_auth.settings_db_path)
        .and_then(|store| store.set(&key, payload.value.trim()))
    {
        Ok(()) => (StatusCode::OK, Json(json!({"ok": true, "key": key}))),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"ok": false, "error": err})),
        ),
    }
}

async fn admin_settings_delete(
    State(state): State<VpsAppState>,
    headers: HeaderMap,
    Query(query): Query<SettingsKeyQuery>,
) -> impl IntoResponse {
    if !admin_authenticated(&headers, &state.admin_auth) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({"ok": false, "error": "admin auth required"})),
        );
    }
    let Some(key) = query.key else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"ok": false, "error": "key query parameter is required"})),
        );
    };
    match SettingsStore::open(&state.admin_auth.settings_db_path)
        .and_then(|store| store.delete(&key))
    {
        Ok(true) => (
            StatusCode::OK,
            Json(json!({"ok": true, "deleted": true, "key": key})),
        ),
        Ok(false) => (
            StatusCode::NOT_FOUND,
            Json(json!({"ok": false, "error": "setting not found"})),
        ),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"ok": false, "error": err})),
        ),
    }
}
