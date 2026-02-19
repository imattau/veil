use std::collections::HashMap;

use rand::RngCore;

use crate::state_store::{GroupKeyRecord, IdentityRecord};
use veil_crypto::signing::{NostrSigner, Signer};

use super::NodeIdentity;

pub(super) fn parse_identity(record: &IdentityRecord) -> Option<NodeIdentity> {
    let public_key = decode_hex_32(&record.public_key_hex)?;
    let secret_key = decode_hex_32(&record.secret_key_hex)?;
    let derived = NostrSigner::from_secret(secret_key).ok()?.public_key();
    if derived != public_key {
        return None;
    }

    let encrypt_key = decode_hex_32(&record.encrypt_key_hex).unwrap_or_else(random_key_material);

    Some(NodeIdentity {
        public_key,
        secret_key,
        encrypt_key,
    })
}

pub(super) fn parse_group_keys(
    records: &[GroupKeyRecord],
) -> HashMap<String, HashMap<String, [u8; 32]>> {
    let mut out: HashMap<String, HashMap<String, [u8; 32]>> = HashMap::new();
    for record in records {
        if record.group_id.trim().is_empty() || record.key_id.trim().is_empty() {
            continue;
        }
        let Some(key) = decode_hex_32(&record.key_hex) else {
            continue;
        };
        out.entry(record.group_id.clone())
            .or_default()
            .insert(record.key_id.clone(), key);
    }
    out
}

pub(super) fn flatten_group_keys(
    group_keys: &HashMap<String, HashMap<String, [u8; 32]>>,
) -> Vec<GroupKeyRecord> {
    let mut out = Vec::new();
    for (group_id, keys) in group_keys {
        for (key_id, key) in keys {
            out.push(GroupKeyRecord {
                group_id: group_id.clone(),
                key_id: key_id.clone(),
                key_hex: hex::encode(key),
                key_enc_nonce_b64: None,
                key_enc_b64: None,
            });
        }
    }
    out
}

pub(super) fn generate_group_key_entry() -> (String, [u8; 32]) {
    (random_key_id(), random_key_material())
}

fn random_key_id() -> String {
    let mut value = [0u8; 8];
    rand::thread_rng().fill_bytes(&mut value);
    hex::encode(value)
}

pub(super) fn decode_hex_32(value: &str) -> Option<[u8; 32]> {
    <[u8; 32] as hex::FromHex>::from_hex(value.trim()).ok()
}

pub(super) fn generate_identity() -> NodeIdentity {
    let (secret_key, signer) = loop {
        let mut secret_key = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut secret_key);
        if let Ok(signer) = NostrSigner::from_secret(secret_key) {
            break (secret_key, signer);
        }
    };
    let public_key = signer.public_key();
    let encrypt_key = veil_crypto::keys::derive_encrypt_key(&secret_key);
    NodeIdentity {
        public_key,
        secret_key,
        encrypt_key,
    }
}

fn random_key_material() -> [u8; 32] {
    let mut key = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut key);
    key
}
