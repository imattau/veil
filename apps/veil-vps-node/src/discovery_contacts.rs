use std::collections::HashMap;

pub(crate) const DISCOVERY_MAX_CONTACTS: usize = 2_048;
pub(crate) const DISCOVERY_GOSSIP_IMPORT_LIMIT: usize = 256;
const DISCOVERY_MAX_PEER_ID_LEN: usize = 128;
const DISCOVERY_MAX_ENDPOINT_LEN: usize = 1024;
const DISCOVERY_MAX_LAN_ADDRS: usize = 8;
const DISCOVERY_MAX_LAN_ADDR_LEN: usize = 256;

pub(crate) fn upsert_discovery_contact(
    table: &mut HashMap<String, veil_android_node::ContactBundle>,
    contact: veil_android_node::ContactBundle,
    max_contacts: usize,
) {
    let keep_peer_id = contact.peer_id.clone();
    table.insert(keep_peer_id.clone(), contact);
    trim_discovery_table(table, max_contacts, Some(&keep_peer_id));
}

pub(crate) fn sanitize_discovery_contact(
    mut contact: veil_android_node::ContactBundle,
) -> Option<veil_android_node::ContactBundle> {
    let peer_id = contact.peer_id.trim();
    if peer_id.is_empty() || peer_id.len() > DISCOVERY_MAX_PEER_ID_LEN {
        return None;
    }
    let pubkey_hex = normalize_pubkey_hex(&contact.pubkey_hex)?;
    contact.peer_id = peer_id.to_string();
    contact.pubkey_hex = pubkey_hex;
    contact.ws_url = sanitize_endpoint(contact.ws_url);
    contact.quic_addr = sanitize_endpoint(contact.quic_addr);
    contact.rpc_url = sanitize_endpoint(contact.rpc_url);
    contact.lan_addrs = contact
        .lan_addrs
        .into_iter()
        .filter_map(|addr| {
            let trimmed = addr.trim();
            if trimmed.is_empty() || trimmed.len() > DISCOVERY_MAX_LAN_ADDR_LEN {
                return None;
            }
            Some(trimmed.to_string())
        })
        .take(DISCOVERY_MAX_LAN_ADDRS)
        .collect();
    Some(contact)
}

pub(crate) fn normalize_pubkey_hex(value: &str) -> Option<String> {
    let canonical = value.trim().to_ascii_lowercase();
    if canonical.len() != 64 || !canonical.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    Some(canonical)
}

fn sanitize_endpoint(value: Option<String>) -> Option<String> {
    value.and_then(|entry| {
        let trimmed = entry.trim();
        if trimmed.is_empty() || trimmed.len() > DISCOVERY_MAX_ENDPOINT_LEN {
            return None;
        }
        Some(trimmed.to_string())
    })
}

fn trim_discovery_table(
    table: &mut HashMap<String, veil_android_node::ContactBundle>,
    max_contacts: usize,
    keep_peer_id: Option<&str>,
) {
    while table.len() > max_contacts {
        let evict_key = table
            .keys()
            .find(|candidate| {
                keep_peer_id
                    .map(|keep| candidate.as_str() != keep)
                    .unwrap_or(true)
            })
            .cloned();
        match evict_key {
            Some(peer_id) => {
                table.remove(&peer_id);
            }
            None => break,
        }
    }
}
