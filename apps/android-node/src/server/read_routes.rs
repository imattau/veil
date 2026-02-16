use super::*;

pub(super) async fn fetch_shard(
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

pub(super) async fn fetch_object(
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

pub(super) async fn events_ws(
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
pub(super) struct EventsQuery {
    since: Option<u64>,
}
