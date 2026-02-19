use std::collections::HashMap;

use crate::api::ContactBundle;

const MAX_DYNAMIC_PEER_ID_LEN: usize = 128;
const MAX_DYNAMIC_ENDPOINT_LEN: usize = 1024;
pub(super) const MAX_DYNAMIC_FAST_PEERS: usize = 512;
pub(super) const MAX_DYNAMIC_FALLBACK_PEERS: usize = 512;
pub(super) const MAX_DYNAMIC_PEER_BINDINGS: usize = 2048;

#[derive(Debug, Default, Clone)]
pub(super) struct DynamicPeerStore {
    fast_peers: Vec<String>,
    fallback_peers: Vec<String>,
    peer_map: HashMap<String, [u8; 32]>,
}

impl DynamicPeerStore {
    pub(super) fn add_contact(&mut self, contact: &ContactBundle) {
        if let Some(quic_addr) = contact
            .quic_addr
            .as_deref()
            .and_then(normalize_dynamic_endpoint)
        {
            push_unique_bounded(&mut self.fast_peers, quic_addr, MAX_DYNAMIC_FAST_PEERS);
        }
        if let Some(ws_url) = contact
            .ws_url
            .as_deref()
            .and_then(normalize_dynamic_endpoint)
        {
            push_unique_bounded(&mut self.fallback_peers, ws_url, MAX_DYNAMIC_FALLBACK_PEERS);
        }
        if let (Some(peer_id), Some(key)) = (
            normalize_dynamic_peer_id(&contact.peer_id),
            decode_peer_pubkey_hex(&contact.pubkey_hex),
        ) {
            upsert_peer_binding(&mut self.peer_map, peer_id, key);
        }
    }

    pub(super) fn replace_from_contacts(&mut self, contacts: &[ContactBundle]) {
        self.fast_peers.clear();
        self.fallback_peers.clear();
        self.peer_map.clear();
        for contact in contacts {
            self.add_contact(contact);
        }
    }

    pub(super) fn fast_peers(&self) -> &[String] {
        &self.fast_peers
    }

    pub(super) fn fallback_peers(&self) -> &[String] {
        &self.fallback_peers
    }

    pub(super) fn peer_map(&self) -> &HashMap<String, [u8; 32]> {
        &self.peer_map
    }

    #[cfg(test)]
    pub(super) fn snapshots(&self) -> (Vec<String>, Vec<String>, HashMap<String, [u8; 32]>) {
        (
            self.fast_peers.clone(),
            self.fallback_peers.clone(),
            self.peer_map.clone(),
        )
    }
}

fn push_unique_bounded(values: &mut Vec<String>, candidate: String, cap: usize) {
    if values.contains(&candidate) || values.len() >= cap {
        return;
    }
    values.push(candidate);
}

fn upsert_peer_binding(map: &mut HashMap<String, [u8; 32]>, peer_id: String, key: [u8; 32]) {
    if map.contains_key(&peer_id) || map.len() < MAX_DYNAMIC_PEER_BINDINGS {
        map.insert(peer_id, key);
    }
}

fn normalize_dynamic_peer_id(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.len() > MAX_DYNAMIC_PEER_ID_LEN {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn normalize_dynamic_endpoint(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.len() > MAX_DYNAMIC_ENDPOINT_LEN {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn decode_peer_pubkey_hex(value: &str) -> Option<[u8; 32]> {
    let mut key = [0u8; 32];
    hex::decode_to_slice(value, &mut key).ok()?;
    Some(key)
}
