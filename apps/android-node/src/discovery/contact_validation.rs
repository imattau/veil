use std::collections::HashSet;

use crate::api::ContactBundle;

pub(super) fn bounded_gossip_contacts(requested: usize, max_contacts: usize) -> usize {
    requested.max(1).min(max_contacts)
}

pub(super) fn normalize_reply_to_peer_id(value: &str, max_peer_id_len: usize) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.len() > max_peer_id_len {
        None
    } else {
        Some(trimmed.to_string())
    }
}

pub(super) fn sanitize_contact(
    mut contact: ContactBundle,
    max_peer_id_len: usize,
    max_endpoint_len: usize,
    max_lan_addr_len: usize,
    max_lan_addrs: usize,
) -> Option<ContactBundle> {
    let peer_id = contact.peer_id.trim();
    if peer_id.is_empty() || peer_id.len() > max_peer_id_len {
        return None;
    }
    if !valid_pubkey_hex(&contact.pubkey_hex) {
        return None;
    }
    contact.peer_id = peer_id.to_string();
    contact.ws_url = sanitize_endpoint(contact.ws_url, max_endpoint_len);
    contact.quic_addr = sanitize_endpoint(contact.quic_addr, max_endpoint_len);
    contact.rpc_url = sanitize_endpoint(contact.rpc_url, max_endpoint_len);
    contact.lan_addrs = contact
        .lan_addrs
        .into_iter()
        .scan(HashSet::new(), |seen, addr| {
            let trimmed = addr.trim();
            if trimmed.is_empty() || trimmed.len() > max_lan_addr_len {
                return Some(None);
            }
            Some(seen.insert(trimmed.to_string()).then(|| trimmed.to_string()))
        })
        .flatten()
        .take(max_lan_addrs)
        .collect();
    Some(contact)
}

fn sanitize_endpoint(value: Option<String>, max_endpoint_len: usize) -> Option<String> {
    value.and_then(|entry| {
        let trimmed = entry.trim();
        if trimmed.is_empty() || trimmed.len() > max_endpoint_len {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

pub(super) fn valid_pubkey_hex(value: &str) -> bool {
    let mut key = [0u8; 32];
    hex::decode_to_slice(value, &mut key).is_ok()
}

#[cfg(test)]
mod tests {
    use super::{sanitize_contact, valid_pubkey_hex};
    use crate::api::ContactBundle;

    #[test]
    fn sanitize_contact_rejects_invalid_pubkey() {
        let contact = ContactBundle {
            peer_id: "peer-a".to_string(),
            ws_url: Some("ws://relay".to_string()),
            quic_addr: Some("127.0.0.1:5000".to_string()),
            pubkey_hex: "xyz".to_string(),
            rpc_url: Some("http://relay".to_string()),
            lan_addrs: vec!["10.0.0.2:9333".to_string()],
        };
        assert!(sanitize_contact(contact, 128, 1024, 256, 8).is_none());
    }

    #[test]
    fn sanitize_contact_trims_and_caps_fields() {
        let contact = ContactBundle {
            peer_id: " peer-a ".to_string(),
            ws_url: Some(" ws://relay ".to_string()),
            quic_addr: Some(" 127.0.0.1:5000 ".to_string()),
            pubkey_hex: "11".repeat(32),
            rpc_url: Some(" https://relay ".to_string()),
            lan_addrs: vec![
                " ".to_string(),
                " 10.0.0.2:9333 ".to_string(),
                "10.0.0.3:9333".to_string(),
            ],
        };
        let sanitized = sanitize_contact(contact, 128, 1024, 256, 1).expect("sanitized");
        assert_eq!(sanitized.peer_id, "peer-a");
        assert_eq!(sanitized.ws_url.as_deref(), Some("ws://relay"));
        assert_eq!(sanitized.quic_addr.as_deref(), Some("127.0.0.1:5000"));
        assert_eq!(sanitized.rpc_url.as_deref(), Some("https://relay"));
        assert_eq!(sanitized.lan_addrs, vec!["10.0.0.2:9333".to_string()]);
    }

    #[test]
    fn sanitize_contact_deduplicates_lan_addrs() {
        let contact = ContactBundle {
            peer_id: "peer-a".to_string(),
            ws_url: None,
            quic_addr: None,
            pubkey_hex: "11".repeat(32),
            rpc_url: None,
            lan_addrs: vec![
                " 10.0.0.2:9333 ".to_string(),
                "10.0.0.2:9333".to_string(),
                "10.0.0.3:9333".to_string(),
            ],
        };
        let sanitized = sanitize_contact(contact, 128, 1024, 256, 8).expect("sanitized");
        assert_eq!(
            sanitized.lan_addrs,
            vec!["10.0.0.2:9333".to_string(), "10.0.0.3:9333".to_string()]
        );
    }

    #[test]
    fn valid_pubkey_hex_only_accepts_64_hex_chars() {
        assert!(valid_pubkey_hex(&"aa".repeat(32)));
        assert!(!valid_pubkey_hex("abcd"));
        assert!(!valid_pubkey_hex(&"zz".repeat(32)));
    }
}
