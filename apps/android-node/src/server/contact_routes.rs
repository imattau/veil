use super::*;

pub(super) async fn contact_list(State(state): State<AppState>, headers: HeaderMap) -> Response {
    if !authorized(&headers, &state.auth_token) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let contacts = state.node.contacts();
    Json(ContactListResponse { contacts }).into_response()
}

pub(super) async fn contact_self(State(state): State<AppState>, headers: HeaderMap) -> Response {
    if !authorized(&headers, &state.auth_token) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let contact = build_self_contact(&state.node, &state.protocol);
    Json(contact).into_response()
}

pub(super) async fn contact_import(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<ContactImportRequest>,
) -> Response {
    if !authorized(&headers, &state.auth_token) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    if !request.contact.pubkey_hex.is_empty() && request.contact.pubkey_hex.len() != 64 {
        return bad_request("invalid_pubkey", "pubkey invalid");
    }
    state.node.set_contact(request.contact);
    let contacts = state.node.contacts();
    state.protocol.sync_contacts(&contacts).await;
    Json(ContactImportResponse { imported: true }).into_response()
}

pub(super) async fn contact_delete(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<ContactDeleteRequest>,
) -> Response {
    if !authorized(&headers, &state.auth_token) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let peer_id = request.peer_id.trim();
    if peer_id.is_empty() {
        return bad_request("invalid_peer_id", "peer id is empty");
    }
    let deleted = state.node.remove_contact(peer_id);
    if deleted {
        let contacts = state.node.contacts();
        state.protocol.sync_contacts(&contacts).await;
    }
    Json(ContactDeleteResponse { deleted }).into_response()
}
