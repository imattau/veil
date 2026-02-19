use std::collections::HashSet;

use veil_codec::object::decode_object_cbor_prefix;
use veil_codec::shard::{decode_shard_cbor, ShardV1};
use veil_core::ObjectRoot;
use veil_crypto::aead::{build_veil_aad, AeadCipher, XChaCha20Poly1305Cipher};
use veil_fec::profile::ErasureCodingMode;
use veil_fec::sharder::{derive_object_root, reconstruct_object_padded_with_mode};
use veil_node::state::NodeState;

use super::erasure_mode_from_shards;

pub(super) struct DecryptedPayload {
    object_root: ObjectRoot,
    payload: Vec<u8>,
}

pub(super) fn reconstruct_payload_for_root(
    state: &NodeState,
    requested_root: ObjectRoot,
    fallback_mode: ErasureCodingMode,
    encrypt_key: [u8; 32],
) -> Option<Vec<u8>> {
    let target_roots = target_wire_roots(state, requested_root);

    // 1. Direct lookup by wire/content root.
    for target_root in &target_roots {
        let shards = shards_for_root(state, *target_root);
        if let Some(payload) =
            reconstruct_decrypted_payload(&shards, *target_root, fallback_mode, encrypt_key)
        {
            if let Some(match_payload) =
                match_payload_for_root(*target_root, requested_root, payload)
            {
                return Some(match_payload);
            }
        }
    }

    // 2. Fallback scan for content hash match inside any other cached object.
    let roots: Vec<_> = state.shard_index.keys().copied().collect();
    for wire_root in roots {
        if target_roots.contains(&wire_root) {
            continue;
        }
        let shards = shards_for_root(state, wire_root);
        if let Some(payload) =
            reconstruct_decrypted_payload(&shards, wire_root, fallback_mode, encrypt_key)
        {
            if let Some(match_payload) = match_payload_for_root(wire_root, requested_root, payload)
            {
                return Some(match_payload);
            }
        }
    }

    None
}

pub(super) fn target_wire_roots(state: &NodeState, root: ObjectRoot) -> HashSet<ObjectRoot> {
    let mut roots = HashSet::new();
    roots.insert(root);
    if let Some(wire_root) = state.content_index.get(&root) {
        roots.insert(*wire_root);
    }
    roots
}

pub(super) fn shards_for_root(state: &NodeState, wire_root: ObjectRoot) -> Vec<ShardV1> {
    let Some(sids) = state.shard_index.get(&wire_root) else {
        return Vec::new();
    };
    let mut shards = Vec::new();
    for sid in sids {
        if let Some(cached) = state.cache.get(sid) {
            if let Ok(shard) = decode_shard_cbor(&cached.bytes) {
                shards.push(shard);
            }
        }
    }
    shards
}

pub(super) fn reconstruct_decrypted_payload(
    shards: &[ShardV1],
    wire_root: ObjectRoot,
    fallback_mode: ErasureCodingMode,
    encrypt_key: [u8; 32],
) -> Option<DecryptedPayload> {
    if shards.is_empty() {
        return None;
    }
    let mode = erasure_mode_from_shards(shards, fallback_mode);
    let reconstructed = reconstruct_object_padded_with_mode(shards, wire_root, mode).ok()?;
    let (object, _) = decode_object_cbor_prefix(&reconstructed).ok()?;
    let aad = build_veil_aad(object.tag, object.namespace, object.epoch);
    let cipher = XChaCha20Poly1305Cipher;
    let payload = cipher
        .decrypt(&encrypt_key, object.nonce, &aad, &object.ciphertext)
        .or_else(|_| cipher.decrypt(&[0u8; 32], object.nonce, &aad, &object.ciphertext))
        .ok()?;

    Some(DecryptedPayload {
        object_root: object.object_root,
        payload,
    })
}

pub(super) fn match_payload_for_root(
    wire_root: ObjectRoot,
    requested_root: ObjectRoot,
    payload: DecryptedPayload,
) -> Option<Vec<u8>> {
    let DecryptedPayload {
        object_root,
        payload,
    } = payload;

    if wire_root == requested_root {
        return Some(payload);
    }

    if object_root == requested_root {
        if let Some(item) = batch_item_for_root(&payload, requested_root) {
            return Some(item);
        }
        return Some(payload);
    }

    batch_item_for_root(&payload, requested_root)
}

fn batch_item_for_root(payload: &[u8], requested_root: ObjectRoot) -> Option<Vec<u8>> {
    let batch = ciborium::de::from_reader::<Vec<Vec<u8>>, _>(payload).ok()?;
    batch
        .into_iter()
        .find(|item| derive_object_root(item) == requested_root)
}
