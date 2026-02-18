use crate::api::{
    ContactBundle, DiscoveryAnnounceRequest, DiscoveryAnnounceResponse, DiscoveryGossipRequest,
    DiscoveryGossipResponse, DiscoveryLookupRequest, DiscoveryLookupResponse,
};
use crate::state::NodeState;

pub(super) fn handle_discovery_announce(
    state: &NodeState,
    request: DiscoveryAnnounceRequest,
    max_neighbors: usize,
    sanitize_contact: fn(ContactBundle) -> Option<ContactBundle>,
) -> DiscoveryAnnounceResponse {
    let Some(contact) = sanitize_contact(request.contact) else {
        return DiscoveryAnnounceResponse {
            accepted: false,
            neighbors: Vec::new(),
        };
    };
    state.add_contact(contact.clone());
    let neighbors = state.discovery_lookup_contact(&contact, max_neighbors);
    DiscoveryAnnounceResponse {
        accepted: true,
        neighbors,
    }
}

pub(super) fn handle_discovery_lookup(
    state: &NodeState,
    request: DiscoveryLookupRequest,
    default_limit: usize,
    max_contacts: usize,
) -> DiscoveryLookupResponse {
    let limit = request.limit.unwrap_or(default_limit).min(max_contacts);
    let mut result = Vec::new();
    if let Some(peer_id) = request.peer_id.as_deref() {
        result = state.discovery_lookup_peer(peer_id, limit);
    } else if let Some(pubkey_hex) = request.pubkey_hex.as_deref() {
        result = state.discovery_lookup_pubkey(pubkey_hex, limit);
    }
    DiscoveryLookupResponse { contacts: result }
}

pub(super) fn handle_discovery_gossip(
    state: &NodeState,
    request: DiscoveryGossipRequest,
    max_contacts: usize,
    sanitize_contact: fn(ContactBundle) -> Option<ContactBundle>,
    bounded_gossip_contacts: impl Fn(usize) -> usize,
) -> DiscoveryGossipResponse {
    for contact in request
        .contacts
        .into_iter()
        .filter_map(sanitize_contact)
        .take(max_contacts)
    {
        state.add_contact(contact);
    }
    DiscoveryGossipResponse {
        contacts: state.discovery_sample(bounded_gossip_contacts(max_contacts)),
    }
}
