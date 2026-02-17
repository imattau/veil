use serde::{Deserialize, Serialize};

use crate::api::ContactBundle;

use super::DISCOVERY_MAX_CONTACTS;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiscoveryKind {
    Announce,
    Lookup,
    Response,
    Gossip,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryMessage {
    pub kind: DiscoveryKind,
    #[serde(default)]
    pub contact: Option<ContactBundle>,
    #[serde(default)]
    pub contacts: Vec<ContactBundle>,
    #[serde(default)]
    pub target_peer_id: Option<String>,
    #[serde(default)]
    pub target_pubkey: Option<String>,
    #[serde(default)]
    pub reply_to: Option<String>,
    #[serde(default)]
    pub ttl: u8,
}

impl DiscoveryMessage {
    pub fn announce(contact: ContactBundle) -> Self {
        Self {
            kind: DiscoveryKind::Announce,
            contact: Some(contact),
            contacts: Vec::new(),
            target_peer_id: None,
            target_pubkey: None,
            reply_to: None,
            ttl: 1,
        }
    }

    pub fn gossip(mut contacts: Vec<ContactBundle>) -> Self {
        contacts.truncate(DISCOVERY_MAX_CONTACTS);
        Self {
            kind: DiscoveryKind::Gossip,
            contact: None,
            contacts,
            target_peer_id: None,
            target_pubkey: None,
            reply_to: None,
            ttl: 1,
        }
    }

    pub fn lookup(peer_id: Option<String>, pubkey_hex: Option<String>, reply_to: String) -> Self {
        Self {
            kind: DiscoveryKind::Lookup,
            contact: None,
            contacts: Vec::new(),
            target_peer_id: peer_id,
            target_pubkey: pubkey_hex,
            reply_to: Some(reply_to),
            ttl: 1,
        }
    }

    pub fn response(contacts: Vec<ContactBundle>, target_peer_id: Option<String>) -> Self {
        Self {
            kind: DiscoveryKind::Response,
            contact: None,
            contacts,
            target_peer_id,
            target_pubkey: None,
            reply_to: None,
            ttl: 1,
        }
    }
}
