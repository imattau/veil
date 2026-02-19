use crate::api::ContactBundle;
use crate::protocol::ProtocolEngine;
use crate::state::NodeState;

use super::{is_local_identity_contact, DiscoveryKind, DiscoveryMessage};

const ANNOUNCE_NEIGHBOR_LIMIT: usize = 12;
const LOOKUP_CONTACT_LIMIT: usize = 16;

pub(super) async fn handle_discovery_message(
    state: &NodeState,
    protocol: &ProtocolEngine,
    msg: DiscoveryMessage,
    max_contacts: usize,
    normalize_reply_to_peer_id: fn(&str) -> Option<String>,
    sanitize_contact: fn(ContactBundle) -> Option<ContactBundle>,
) -> Option<()> {
    match msg.kind {
        DiscoveryKind::Announce => {
            let contact = sanitize_contact(msg.contact?)?;
            let local_peer_id = protocol.peer_id();
            let local_pubkey = state.identity().public_key;
            if is_local_identity_contact(&contact, &local_peer_id, local_pubkey) {
                return None;
            }
            state.add_contact(contact.clone());
            protocol.add_contact(&contact).await;
            let neighbors = state.discovery_lookup_contact(&contact, ANNOUNCE_NEIGHBOR_LIMIT);
            let response = DiscoveryMessage::response(neighbors, Some(contact.peer_id));
            let _ = protocol.publish_discovery(response).await;
        }
        DiscoveryKind::Lookup => {
            let reply_to = msg
                .reply_to
                .as_deref()
                .and_then(normalize_reply_to_peer_id)?;
            let contacts = if let Some(peer_id) = msg.target_peer_id.as_deref() {
                state.discovery_lookup_peer(peer_id, LOOKUP_CONTACT_LIMIT)
            } else if let Some(pubkey_hex) = msg.target_pubkey.as_deref() {
                state.discovery_lookup_pubkey(pubkey_hex, LOOKUP_CONTACT_LIMIT)
            } else {
                Vec::new()
            };
            if !contacts.is_empty() {
                let response = DiscoveryMessage::response(contacts, Some(reply_to));
                let _ = protocol.publish_discovery(response).await;
            }
        }
        DiscoveryKind::Response => {
            if let Some(target_peer_id) = msg.target_peer_id.as_deref() {
                if target_peer_id != protocol.peer_id() {
                    return None;
                }
            }
            let local_peer_id = protocol.peer_id();
            let local_pubkey = state.identity().public_key;
            ingest_contacts(
                state,
                protocol,
                msg.contacts,
                max_contacts,
                &local_peer_id,
                local_pubkey,
                sanitize_contact,
            )
            .await;
        }
        DiscoveryKind::Gossip => {
            let local_peer_id = protocol.peer_id();
            let local_pubkey = state.identity().public_key;
            ingest_contacts(
                state,
                protocol,
                msg.contacts,
                max_contacts,
                &local_peer_id,
                local_pubkey,
                sanitize_contact,
            )
            .await;
        }
    }
    Some(())
}

async fn ingest_contacts(
    state: &NodeState,
    protocol: &ProtocolEngine,
    contacts: Vec<ContactBundle>,
    max_contacts: usize,
    local_peer_id: &str,
    local_pubkey: [u8; 32],
    sanitize_contact: fn(ContactBundle) -> Option<ContactBundle>,
) {
    for contact in contacts
        .into_iter()
        .filter_map(sanitize_contact)
        .take(max_contacts)
    {
        if is_local_identity_contact(&contact, local_peer_id, local_pubkey) {
            continue;
        }
        state.add_contact(contact.clone());
        protocol.add_contact(&contact).await;
    }
}
