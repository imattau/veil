use super::*;

use crate::api::ContactBundle;
use crate::discovery::{
    handle_discovery_announce, handle_discovery_gossip, handle_discovery_lookup,
    sanitize_discovery_contact, DiscoveryMessage,
};

const MAX_DISCOVERY_CONTACTS_PER_REQUEST: usize = 64;
const MAX_DISCOVERY_LOOKUP_PEER_ID_LEN: usize = 128;

pub(super) async fn discovery_announce(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<DiscoveryAnnounceRequest>,
) -> Response {
    if !authorized(&headers, &state.auth_token) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let Some(contact) = sanitize_discovery_contact(request.contact) else {
        return bad_request("invalid_contact", "contact is invalid");
    };
    state.protocol.add_contact(&contact).await;
    let response = handle_discovery_announce(
        &state.node,
        DiscoveryAnnounceRequest {
            contact: contact.clone(),
        },
        16,
    );
    for contact in &response.neighbors {
        state.protocol.add_contact(contact).await;
    }
    let _ = state
        .protocol
        .publish_discovery(DiscoveryMessage::announce(contact))
        .await;
    Json(response).into_response()
}

pub(super) async fn discovery_lookup(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<DiscoveryLookupRequest>,
) -> Response {
    if !authorized(&headers, &state.auth_token) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let lookup = match sanitize_discovery_lookup_request(request) {
        Ok(value) => value,
        Err(response) => return response,
    };
    let response: DiscoveryLookupResponse = handle_discovery_lookup(&state.node, lookup.clone());
    let reply_to = state.protocol.peer_id();
    let msg = DiscoveryMessage::lookup(lookup.peer_id, lookup.pubkey_hex, reply_to);
    let _ = state.protocol.publish_discovery(msg).await;
    Json(response).into_response()
}

pub(super) async fn discovery_gossip(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<DiscoveryGossipRequest>,
) -> Response {
    if !authorized(&headers, &state.auth_token) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let contacts: Vec<ContactBundle> = request
        .contacts
        .into_iter()
        .filter_map(sanitize_discovery_contact)
        .take(MAX_DISCOVERY_CONTACTS_PER_REQUEST)
        .collect();
    for contact in &contacts {
        state.protocol.add_contact(contact).await;
    }
    let response: DiscoveryGossipResponse = handle_discovery_gossip(
        &state.node,
        DiscoveryGossipRequest {
            contacts: contacts.clone(),
        },
        24,
    );
    let msg = DiscoveryMessage::gossip(contacts);
    let _ = state.protocol.publish_discovery(msg).await;
    Json(response).into_response()
}

fn sanitize_discovery_lookup_request(
    request: DiscoveryLookupRequest,
) -> Result<DiscoveryLookupRequest, Response> {
    let peer_id = request.peer_id.and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    });
    let pubkey_hex = request.pubkey_hex.and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    });
    if peer_id.is_none() && pubkey_hex.is_none() {
        return Err(bad_request(
            "invalid_lookup",
            "lookup requires peer_id or pubkey_hex",
        ));
    }
    if let Some(peer_id) = &peer_id {
        if peer_id.len() > MAX_DISCOVERY_LOOKUP_PEER_ID_LEN {
            return Err(bad_request("invalid_peer_id", "peer id is too long"));
        }
    }
    if let Some(pubkey_hex) = &pubkey_hex {
        if !valid_pubkey_hex(pubkey_hex) {
            return Err(bad_request("invalid_pubkey", "pubkey invalid"));
        }
    }
    Ok(DiscoveryLookupRequest {
        peer_id,
        pubkey_hex,
        limit: request.limit,
    })
}
