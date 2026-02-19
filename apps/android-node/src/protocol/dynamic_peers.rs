use std::collections::{hash_map::Entry, HashMap, HashSet};
use std::net::SocketAddr;

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
    fast_peer_set: HashSet<String>,
    fallback_peer_set: HashSet<String>,
    peer_map: HashMap<String, [u8; 32]>,
}

impl DynamicPeerStore {
    pub(super) fn add_contact(&mut self, contact: &ContactBundle) {
        if let Some(quic_addr) = contact
            .quic_addr
            .as_deref()
            .and_then(normalize_dynamic_quic_addr)
        {
            push_unique_bounded(
                &mut self.fast_peers,
                &mut self.fast_peer_set,
                quic_addr,
                MAX_DYNAMIC_FAST_PEERS,
            );
        }
        if let Some(ws_url) = contact
            .ws_url
            .as_deref()
            .and_then(normalize_dynamic_ws_url)
        {
            push_unique_bounded(
                &mut self.fallback_peers,
                &mut self.fallback_peer_set,
                ws_url,
                MAX_DYNAMIC_FALLBACK_PEERS,
            );
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
        self.fast_peer_set.clear();
        self.fallback_peer_set.clear();
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

fn push_unique_bounded(
    values: &mut Vec<String>,
    index: &mut HashSet<String>,
    candidate: String,
    cap: usize,
) {
    if values.len() >= cap || !index.insert(candidate.clone()) {
        return;
    }
    values.push(candidate);
}

fn upsert_peer_binding(map: &mut HashMap<String, [u8; 32]>, peer_id: String, key: [u8; 32]) {
    let len = map.len();
    match map.entry(peer_id) {
        Entry::Occupied(mut entry) => {
            entry.insert(key);
        }
        Entry::Vacant(entry) => {
            if len < MAX_DYNAMIC_PEER_BINDINGS {
                entry.insert(key);
            }
        }
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

fn normalize_dynamic_ws_url(value: &str) -> Option<String> {
    let normalized = normalize_dynamic_endpoint(value)?;
    let parsed = reqwest::Url::parse(&normalized).ok()?;
    if !matches!(parsed.scheme(), "ws" | "wss") || parsed.host_str().is_none() {
        return None;
    }
    Some(normalized)
}

fn normalize_dynamic_quic_addr(value: &str) -> Option<String> {
    let normalized = normalize_dynamic_endpoint(value)?;
    if normalized.parse::<SocketAddr>().is_ok() {
        return Some(normalized);
    }

    if normalized.contains("://") {
        let parsed = reqwest::Url::parse(&normalized).ok()?;
        if parsed.scheme() != "quic" {
            return None;
        }
        if parsed.host_str().is_some() && parsed.port().is_some() {
            return Some(normalized);
        }
        return None;
    }

    let with_scheme = format!("quic://{normalized}");
    let parsed = reqwest::Url::parse(&with_scheme).ok()?;
    if parsed.host_str().is_some() && parsed.port().is_some() {
        Some(normalized)
    } else {
        None
    }
}

fn decode_peer_pubkey_hex(value: &str) -> Option<[u8; 32]> {
    let mut key = [0u8; 32];
    hex::decode_to_slice(value, &mut key).ok()?;
    Some(key)
}

#[cfg(test)]
mod tests {
    use super::{DynamicPeerStore, MAX_DYNAMIC_PEER_BINDINGS};
    use crate::api::ContactBundle;

    fn contact(peer_id: &str, ws_url: &str, quic_addr: &str, pubkey_byte: u8) -> ContactBundle {
        ContactBundle {
            peer_id: peer_id.to_string(),
            ws_url: Some(ws_url.to_string()),
            quic_addr: Some(quic_addr.to_string()),
            pubkey_hex: format!("{:02x}", pubkey_byte).repeat(32),
            rpc_url: None,
            lan_addrs: Vec::new(),
        }
    }

    #[test]
    fn add_contact_deduplicates_endpoints_and_preserves_order() {
        let mut store = DynamicPeerStore::default();

        store.add_contact(&contact("peer-a", "ws://a/ws", "127.0.0.1:9001", 0x11));
        store.add_contact(&contact("peer-b", "ws://b/ws", "127.0.0.1:9002", 0x22));
        store.add_contact(&contact(
            "peer-a-updated",
            "ws://a/ws",
            "127.0.0.1:9001",
            0x33,
        ));

        let (fast, fallback, _) = store.snapshots();
        assert_eq!(
            fast,
            vec!["127.0.0.1:9001".to_string(), "127.0.0.1:9002".to_string()]
        );
        assert_eq!(
            fallback,
            vec!["ws://a/ws".to_string(), "ws://b/ws".to_string()]
        );
    }

    #[test]
    fn replace_from_contacts_rebuilds_membership_indexes() {
        let mut store = DynamicPeerStore::default();
        store.add_contact(&contact("peer-old", "ws://old/ws", "127.0.0.1:9100", 0x10));

        let replacement = vec![
            contact("peer-new-a", "ws://new-a/ws", "127.0.0.1:9201", 0x20),
            contact("peer-new-b", "ws://new-b/ws", "127.0.0.1:9202", 0x21),
        ];
        store.replace_from_contacts(&replacement);

        let (fast, fallback, map) = store.snapshots();
        assert_eq!(
            fast,
            vec!["127.0.0.1:9201".to_string(), "127.0.0.1:9202".to_string()]
        );
        assert_eq!(
            fallback,
            vec!["ws://new-a/ws".to_string(), "ws://new-b/ws".to_string()]
        );
        assert_eq!(map.len(), 2);
        assert!(map.contains_key("peer-new-a"));
        assert!(map.contains_key("peer-new-b"));
    }

    #[test]
    fn upsert_updates_existing_binding_even_when_capacity_reached() {
        let mut store = DynamicPeerStore::default();
        for index in 0..MAX_DYNAMIC_PEER_BINDINGS {
            store.add_contact(&ContactBundle {
                peer_id: format!("peer-{index}"),
                ws_url: None,
                quic_addr: None,
                pubkey_hex: format!("{:064x}", index + 1),
                rpc_url: None,
                lan_addrs: Vec::new(),
            });
        }

        store.add_contact(&ContactBundle {
            peer_id: "peer-0".to_string(),
            ws_url: None,
            quic_addr: None,
            pubkey_hex: "aa".repeat(32),
            rpc_url: None,
            lan_addrs: Vec::new(),
        });
        store.add_contact(&ContactBundle {
            peer_id: "peer-over-cap".to_string(),
            ws_url: None,
            quic_addr: None,
            pubkey_hex: "bb".repeat(32),
            rpc_url: None,
            lan_addrs: Vec::new(),
        });

        let (_, _, map) = store.snapshots();
        assert_eq!(map.len(), MAX_DYNAMIC_PEER_BINDINGS);
        assert_eq!(map.get("peer-0"), Some(&[0xAA; 32]));
        assert!(!map.contains_key("peer-over-cap"));
    }

    #[test]
    fn add_contact_rejects_invalid_transport_endpoints() {
        let mut store = DynamicPeerStore::default();
        store.add_contact(&ContactBundle {
            peer_id: "peer-a".to_string(),
            ws_url: Some("https://relay.example/ws".to_string()),
            quic_addr: Some("not-an-addr".to_string()),
            pubkey_hex: "11".repeat(32),
            rpc_url: None,
            lan_addrs: Vec::new(),
        });

        let (fast, fallback, map) = store.snapshots();
        assert!(fast.is_empty());
        assert!(fallback.is_empty());
        assert_eq!(map.len(), 1);
        assert!(map.contains_key("peer-a"));
    }

    #[test]
    fn add_contact_accepts_quic_domain_with_or_without_scheme() {
        let mut store = DynamicPeerStore::default();
        store.add_contact(&ContactBundle {
            peer_id: "peer-a".to_string(),
            ws_url: None,
            quic_addr: Some("relay.example:9443".to_string()),
            pubkey_hex: "11".repeat(32),
            rpc_url: None,
            lan_addrs: Vec::new(),
        });
        store.add_contact(&ContactBundle {
            peer_id: "peer-b".to_string(),
            ws_url: None,
            quic_addr: Some("quic://relay-2.example:9443".to_string()),
            pubkey_hex: "22".repeat(32),
            rpc_url: None,
            lan_addrs: Vec::new(),
        });

        let (fast, _, _) = store.snapshots();
        assert_eq!(
            fast,
            vec![
                "relay.example:9443".to_string(),
                "quic://relay-2.example:9443".to_string(),
            ]
        );
    }
}
