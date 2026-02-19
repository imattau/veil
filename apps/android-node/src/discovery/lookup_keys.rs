use crate::api::ContactBundle;

pub(super) fn contact_key(contact: &ContactBundle) -> [u8; 32] {
    if let Some(key) = decode_pubkey_hex(&contact.pubkey_hex) {
        return key;
    }
    blake3::hash(contact.peer_id.as_bytes()).into()
}

pub(super) fn key_for_peer(peer_id: &str) -> [u8; 32] {
    blake3::hash(peer_id.as_bytes()).into()
}

pub(super) fn key_for_pubkey(pubkey_hex: &str) -> Option<[u8; 32]> {
    decode_pubkey_hex(pubkey_hex)
}

pub(super) fn xor_distance(a: &[u8; 32], b: &[u8; 32]) -> [u8; 32] {
    let mut out = [0u8; 32];
    for i in 0..32 {
        out[i] = a[i] ^ b[i];
    }
    out
}

fn decode_pubkey_hex(pubkey_hex: &str) -> Option<[u8; 32]> {
    let mut out = [0u8; 32];
    hex::decode_to_slice(pubkey_hex, &mut out).ok()?;
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::{contact_key, key_for_peer, key_for_pubkey, xor_distance};
    use crate::api::ContactBundle;

    fn make_contact(peer_id: &str, pubkey_hex: &str) -> ContactBundle {
        ContactBundle {
            peer_id: peer_id.to_string(),
            ws_url: None,
            quic_addr: None,
            pubkey_hex: pubkey_hex.to_string(),
            rpc_url: None,
            lan_addrs: Vec::new(),
        }
    }

    #[test]
    fn contact_key_prefers_valid_pubkey_bytes() {
        let contact = make_contact("peer-a", &"11".repeat(32));
        let key = contact_key(&contact);
        assert_eq!(key, [0x11; 32]);
    }

    #[test]
    fn contact_key_falls_back_to_peer_id_hash() {
        let contact = make_contact("peer-b", "not-a-pubkey");
        let expected: [u8; 32] = blake3::hash(b"peer-b").into();
        assert_eq!(contact_key(&contact), expected);
    }

    #[test]
    fn key_for_pubkey_requires_exact_hex_len() {
        assert!(key_for_pubkey("abcd").is_none());
        assert!(key_for_pubkey(&"zz".repeat(32)).is_none());
        assert_eq!(key_for_pubkey(&"22".repeat(32)), Some([0x22; 32]));
    }

    #[test]
    fn xor_distance_is_bytewise_xor() {
        let left = key_for_peer("alpha");
        let right = key_for_peer("beta");
        let distance = xor_distance(&left, &right);
        for i in 0..32 {
            assert_eq!(distance[i], left[i] ^ right[i]);
        }
    }
}
