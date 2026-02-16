use super::*;

pub(super) async fn health(State(state): State<AppState>) -> Response {
    Json(HealthResponse {
        status: "ok".to_string(),
        version: state.version.clone(),
    })
    .into_response()
}

pub(super) async fn status(State(state): State<AppState>, headers: HeaderMap) -> Response {
    if !authorized(&headers, &state.auth_token) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let payload: StatusResponse = state.node.status();
    Json(payload).into_response()
}

pub(super) async fn feed(State(state): State<AppState>, headers: HeaderMap) -> Response {
    if !authorized(&headers, &state.auth_token) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let events = state.node.get_feed(50);
    Json(FeedResponse { events }).into_response()
}

pub(super) async fn subscriptions(State(state): State<AppState>, headers: HeaderMap) -> Response {
    if !authorized(&headers, &state.auth_token) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let subscriptions = state.node.get_subscriptions();
    Json(SubscriptionListResponse { subscriptions }).into_response()
}

pub(super) async fn identity(State(state): State<AppState>, headers: HeaderMap) -> Response {
    if !authorized(&headers, &state.auth_token) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let identity = state.node.identity();
    Json(IdentityResponse {
        public_key_hex: identity.public_key_hex(),
    })
    .into_response()
}

pub(super) async fn rotate_identity(State(state): State<AppState>, headers: HeaderMap) -> Response {
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

pub(super) async fn export_identity(State(state): State<AppState>, headers: HeaderMap) -> Response {
    if !authorized(&headers, &state.auth_token) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    if !state.allow_identity_export {
        return (
            StatusCode::FORBIDDEN,
            Json(ErrorResponse {
                code: "identity_export_disabled".to_string(),
                message: "identity export is disabled".to_string(),
            }),
        )
            .into_response();
    }
    let (public_key_hex, secret_key_hex) = state.node.export_identity();
    Json(IdentityExportResponse {
        public_key_hex,
        secret_key_hex,
    })
    .into_response()
}

pub(super) async fn import_identity(
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

pub(super) async fn subscribe(
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

pub(super) async fn unsubscribe(
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
