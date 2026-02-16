use super::*;

pub(super) async fn policy_summary(State(state): State<AppState>, headers: HeaderMap) -> Response {
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

pub(super) async fn policy_lists(State(state): State<AppState>, headers: HeaderMap) -> Response {
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

pub(super) async fn policy_explain(
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

pub(super) async fn update_policy_config(
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

pub(super) async fn policy_trust(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<PolicySetRequest>,
) -> Response {
    apply_policy_set(state, headers, request, |state, pubkey| {
        state.node.trust_pubkey(pubkey);
    })
    .await
}

pub(super) async fn policy_untrust(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<PolicySetRequest>,
) -> Response {
    apply_policy_set(state, headers, request, |state, pubkey| {
        state.node.untrust_pubkey(pubkey);
    })
    .await
}

pub(super) async fn policy_mute(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<PolicySetRequest>,
) -> Response {
    apply_policy_set(state, headers, request, |state, pubkey| {
        state.node.mute_pubkey(pubkey);
    })
    .await
}

pub(super) async fn policy_unmute(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<PolicySetRequest>,
) -> Response {
    apply_policy_set(state, headers, request, |state, pubkey| {
        state.node.unmute_pubkey(pubkey);
    })
    .await
}

pub(super) async fn policy_block(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<PolicySetRequest>,
) -> Response {
    apply_policy_set(state, headers, request, |state, pubkey| {
        state.node.block_pubkey(pubkey);
    })
    .await
}

pub(super) async fn policy_unblock(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<PolicySetRequest>,
) -> Response {
    apply_policy_set(state, headers, request, |state, pubkey| {
        state.node.unblock_pubkey(pubkey);
    })
    .await
}

async fn apply_policy_set<F>(
    state: AppState,
    headers: HeaderMap,
    request: PolicySetRequest,
    apply: F,
) -> Response
where
    F: FnOnce(&AppState, [u8; 32]),
{
    if !authorized(&headers, &state.auth_token) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    if !valid_pubkey_hex(&request.pubkey_hex) {
        return bad_request("invalid_pubkey", "pubkey invalid");
    }
    let pubkey = hex_to_pubkey(&request.pubkey_hex);
    apply(&state, pubkey);
    state
        .protocol
        .update_wot_policy(state.node.wot_policy())
        .await;
    Json(PolicySetResponse { applied: true }).into_response()
}
