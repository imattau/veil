use crate::api::ContactBundle;
use crate::protocol::ProtocolEngine;
use crate::state::NodeState;

use super::{DiscoveryKind, DiscoveryMessage};

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
            if contact.peer_id == protocol.peer_id() {
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
            ingest_contacts(
                state,
                protocol,
                msg.contacts,
                max_contacts,
                sanitize_contact,
            )
            .await;
        }
        DiscoveryKind::Gossip => {
            ingest_contacts(
                state,
                protocol,
                msg.contacts,
                max_contacts,
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
    sanitize_contact: fn(ContactBundle) -> Option<ContactBundle>,
) {
    for contact in contacts
        .into_iter()
        .filter_map(sanitize_contact)
        .take(max_contacts)
    {
        state.add_contact(contact.clone());
        protocol.add_contact(&contact).await;
    }
}
