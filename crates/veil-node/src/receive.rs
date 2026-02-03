use thiserror::Error;
use veil_codec::error::CodecError;
use veil_codec::object::{
    decode_object_cbor_prefix, object_signature_message_digest, OBJECT_FLAG_SIGNED,
};
use veil_codec::shard::{encode_shard_cbor, ShardV1};
use veil_core::{Epoch, Namespace, ObjectRoot, Tag};
use veil_crypto::aead::{build_veil_aad, AeadCipher, AeadError};
use veil_crypto::signing::{SigningError, Verifier};
use veil_fec::sharder::{reconstruct_object_padded, shard_id, FecError};

use crate::cache::{cache_put, cache_put_with_policy};
use crate::forwarding::should_forward;
use crate::policy::{TrustTier, WotPolicy};
use crate::state::NodeState;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReceiveEvent {
    /// Shard already cached; ignored.
    IgnoredDuplicate,
    /// Tag not subscribed locally; ignored.
    IgnoredNotSubscribed,
    /// Shard buffered but object is not yet reconstructable.
    Buffered {
        object_root: ObjectRoot,
        have: usize,
        need: usize,
    },
    /// Object reconstructed, verified, decrypted, and delivered.
    Delivered {
        object_root: ObjectRoot,
        payload: Vec<u8>,
        namespace: Namespace,
        epoch: Epoch,
        tag: Tag,
        flags: u16,
    },
}

#[derive(Debug, Error)]
pub enum ReceiveError {
    #[error("fec error: {0}")]
    Fec(#[from] FecError),
    #[error("codec error: {0}")]
    Codec(#[from] CodecError),
    #[error("aead error: {0}")]
    Aead(#[from] AeadError),
    #[error("signing error: {0}")]
    Signing(#[from] SigningError),
    #[error("signed object missing sender/signature fields")]
    MissingSignatureFields,
    #[error("signature verification failed")]
    SignatureInvalid,
}

pub struct ReceiveCachePolicy<'a> {
    /// Trust tier applied to inbound shard cache accounting.
    pub tier: TrustTier,
    /// Global cache size cap used for policy-aware insertions.
    pub max_cache_shards: usize,
    /// WoT policy used for storage budget and eviction priority.
    pub wot_policy: &'a dyn WotPolicy,
}

/// Decodes a queue-batched app payload into its original item list.
pub fn decode_batched_payload(payload: &[u8]) -> Result<Vec<Vec<u8>>, serde_cbor::Error> {
    serde_cbor::from_slice(payload)
}

/// Processes a single inbound shard with default cache insertion behavior.
pub fn receive_shard(
    node: &mut NodeState,
    shard: &ShardV1,
    now_step: u64,
    ttl_steps: u64,
    decrypt_key: &[u8; 32],
    cipher: &impl AeadCipher,
    verifier: &impl Verifier,
) -> Result<ReceiveEvent, ReceiveError> {
    receive_shard_with_policy(
        node,
        shard,
        now_step,
        ttl_steps,
        decrypt_key,
        cipher,
        verifier,
        None,
    )
}

/// Processes a single inbound shard with optional policy-aware cache behavior.
#[allow(clippy::too_many_arguments)]
pub fn receive_shard_with_policy(
    node: &mut NodeState,
    shard: &ShardV1,
    now_step: u64,
    ttl_steps: u64,
    decrypt_key: &[u8; 32],
    cipher: &impl AeadCipher,
    verifier: &impl Verifier,
    cache_policy: Option<ReceiveCachePolicy<'_>>,
) -> Result<ReceiveEvent, ReceiveError> {
    let sid = shard_id(shard)?;
    if !should_forward(node, sid, &shard.header.tag) {
        if node.cache.contains_key(&sid) {
            return Ok(ReceiveEvent::IgnoredDuplicate);
        }
        return Ok(ReceiveEvent::IgnoredNotSubscribed);
    }

    let encoded_shard = encode_shard_cbor(shard)?;
    match cache_policy {
        Some(p) => cache_put_with_policy(
            node,
            sid,
            encoded_shard,
            now_step,
            ttl_steps,
            p.tier,
            p.max_cache_shards,
            p.wot_policy,
        ),
        None => cache_put(node, sid, encoded_shard, now_step, ttl_steps),
    }

    let root = shard.header.object_root;
    let root_inbox = node.inbox.entry(root).or_default();
    root_inbox.insert(shard.header.index, shard.clone());

    let have = root_inbox.len();
    let need = shard.header.k as usize;
    if have < need {
        return Ok(ReceiveEvent::Buffered {
            object_root: root,
            have,
            need,
        });
    }

    let collected: Vec<_> = root_inbox.values().cloned().collect();
    let reconstructed = reconstruct_object_padded(&collected, root)?;
    let (object, _) = decode_object_cbor_prefix(&reconstructed)?;

    if (object.flags & OBJECT_FLAG_SIGNED) != 0 {
        let pubkey = object
            .sender_pubkey
            .ok_or(ReceiveError::MissingSignatureFields)?;
        let sig = object
            .signature
            .as_ref()
            .ok_or(ReceiveError::MissingSignatureFields)?
            .0;
        let digest = object_signature_message_digest(&object)?;
        let sig_ok = verifier.verify(pubkey, &digest, sig)?;
        if !sig_ok {
            return Err(ReceiveError::SignatureInvalid);
        }
    }

    let aad = build_veil_aad(object.tag, object.namespace, object.epoch);
    let payload = cipher.decrypt(decrypt_key, object.nonce, &aad, &object.ciphertext)?;

    node.inbox.remove(&root);

    Ok(ReceiveEvent::Delivered {
        object_root: root,
        payload,
        namespace: object.namespace,
        epoch: object.epoch,
        tag: object.tag,
        flags: object.flags,
    })
}

#[cfg(test)]
mod tests {
    use veil_codec::object::{
        encode_object_cbor, object_signature_message_digest, ObjectV1, Signature,
        OBJECT_FLAG_SIGNED, OBJECT_V1_VERSION,
    };
    use veil_core::{Epoch, Namespace};
    use veil_crypto::aead::{build_veil_aad, AeadCipher, XChaCha20Poly1305Cipher};
    use veil_crypto::signing::{Ed25519Signer, Ed25519Verifier, Signer};
    use veil_fec::sharder::{derive_object_root, object_to_shards};

    use super::{
        decode_batched_payload, receive_shard, receive_shard_with_policy, ReceiveCachePolicy,
        ReceiveEvent,
    };
    use crate::policy::{LocalWotPolicy, TrustTier, WotConfig};
    use crate::state::NodeState;

    fn make_signed_encrypted_object(
        payload: &[u8],
        tag: [u8; 32],
        namespace: Namespace,
        epoch: Epoch,
        key: &[u8; 32],
    ) -> Vec<u8> {
        let cipher = XChaCha20Poly1305Cipher;
        let nonce = [0x55_u8; 24];
        let aad = build_veil_aad(tag, namespace, epoch);
        let envelope = cipher
            .encrypt(key, nonce, &aad, payload)
            .expect("encrypt should work");

        let payload_root = derive_object_root(payload);
        let signer = Ed25519Signer::from_secret([0x42_u8; 32]);
        let mut object = ObjectV1 {
            version: OBJECT_V1_VERSION,
            namespace,
            epoch,
            flags: OBJECT_FLAG_SIGNED,
            tag,
            object_root: payload_root,
            sender_pubkey: Some(signer.public_key()),
            signature: Some(Signature([0_u8; 64])),
            nonce: envelope.nonce,
            ciphertext: envelope.ciphertext,
            padding: vec![0_u8; 8],
        };
        let digest = object_signature_message_digest(&object).expect("digest should compute");
        object.signature = Some(Signature(
            signer.sign(&digest).expect("signature should compute"),
        ));
        encode_object_cbor(&object).expect("object should encode")
    }

    #[test]
    fn delivers_after_receiving_k_shards() {
        let mut node = NodeState::default();
        let tag = [0x10_u8; 32];
        node.subscriptions.insert(tag);

        let namespace = Namespace(7);
        let epoch = Epoch(9);
        let decrypt_key = [0xAA_u8; 32];
        let payload = b"hello subscriber".to_vec();
        let encoded_object =
            make_signed_encrypted_object(&payload, tag, namespace, epoch, &decrypt_key);
        let wire_root = derive_object_root(&encoded_object);
        let shards = object_to_shards(&encoded_object, namespace, epoch, tag, wire_root)
            .expect("object should shard");
        let k = shards[0].header.k as usize;

        let cipher = XChaCha20Poly1305Cipher;
        let verifier = Ed25519Verifier;

        let mut last = ReceiveEvent::IgnoredNotSubscribed;
        for shard in shards.iter().take(k) {
            last = receive_shard(&mut node, shard, 10, 30, &decrypt_key, &cipher, &verifier)
                .expect("receive should work");
        }

        match last {
            ReceiveEvent::Delivered {
                object_root,
                payload: delivered,
                ..
            } => {
                assert_eq!(object_root, wire_root);
                assert_eq!(delivered, payload);
            }
            other => panic!("expected Delivered event, got {other:?}"),
        }
        assert!(!node.inbox.contains_key(&wire_root));
    }

    #[test]
    fn ignores_when_not_subscribed() {
        let mut node = NodeState::default();
        let tag = [0x20_u8; 32];
        let namespace = Namespace(1);
        let epoch = Epoch(1);
        let decrypt_key = [0xBB_u8; 32];
        let encoded_object =
            make_signed_encrypted_object(b"payload", tag, namespace, epoch, &decrypt_key);
        let wire_root = derive_object_root(&encoded_object);
        let shard = object_to_shards(&encoded_object, namespace, epoch, tag, wire_root)
            .expect("object should shard")
            .remove(0);

        let event = receive_shard(
            &mut node,
            &shard,
            1,
            5,
            &decrypt_key,
            &XChaCha20Poly1305Cipher,
            &Ed25519Verifier,
        )
        .expect("receive should run");
        assert_eq!(event, ReceiveEvent::IgnoredNotSubscribed);
    }

    #[test]
    fn duplicate_shard_is_ignored() {
        let mut node = NodeState::default();
        let tag = [0x30_u8; 32];
        node.subscriptions.insert(tag);
        let namespace = Namespace(3);
        let epoch = Epoch(3);
        let decrypt_key = [0xCC_u8; 32];
        let encoded_object =
            make_signed_encrypted_object(b"payload", tag, namespace, epoch, &decrypt_key);
        let wire_root = derive_object_root(&encoded_object);
        let shard = object_to_shards(&encoded_object, namespace, epoch, tag, wire_root)
            .expect("object should shard")
            .remove(0);

        let first = receive_shard(
            &mut node,
            &shard,
            1,
            5,
            &decrypt_key,
            &XChaCha20Poly1305Cipher,
            &Ed25519Verifier,
        )
        .expect("receive should run");
        assert!(matches!(first, ReceiveEvent::Buffered { .. }));

        let second = receive_shard(
            &mut node,
            &shard,
            2,
            5,
            &decrypt_key,
            &XChaCha20Poly1305Cipher,
            &Ed25519Verifier,
        )
        .expect("receive should run");
        assert_eq!(second, ReceiveEvent::IgnoredDuplicate);
    }

    #[test]
    fn receive_with_policy_respects_cache_limits() {
        let mut node = NodeState::default();
        let tag = [0x50_u8; 32];
        node.subscriptions.insert(tag);
        let namespace = Namespace(5);
        let epoch = Epoch(5);
        let decrypt_key = [0xDD_u8; 32];

        let cfg = WotConfig {
            unknown_storage_budget: 1,
            ..WotConfig::default()
        };
        let wot_policy = LocalWotPolicy::new(cfg);
        let cache_policy = ReceiveCachePolicy {
            tier: TrustTier::Unknown,
            max_cache_shards: 1,
            wot_policy: &wot_policy,
        };

        let first_obj =
            make_signed_encrypted_object(b"payload-a", tag, namespace, epoch, &decrypt_key);
        let first_root = derive_object_root(&first_obj);
        let first_shard = object_to_shards(&first_obj, namespace, epoch, tag, first_root)
            .expect("object should shard")
            .remove(0);

        let second_obj =
            make_signed_encrypted_object(b"payload-b", tag, namespace, epoch, &decrypt_key);
        let second_root = derive_object_root(&second_obj);
        let second_shard = object_to_shards(&second_obj, namespace, epoch, tag, second_root)
            .expect("object should shard")
            .remove(0);

        let _ = receive_shard_with_policy(
            &mut node,
            &first_shard,
            1,
            100,
            &decrypt_key,
            &XChaCha20Poly1305Cipher,
            &Ed25519Verifier,
            Some(ReceiveCachePolicy {
                tier: cache_policy.tier,
                max_cache_shards: cache_policy.max_cache_shards,
                wot_policy: cache_policy.wot_policy,
            }),
        )
        .expect("receive should work");
        let _ = receive_shard_with_policy(
            &mut node,
            &second_shard,
            2,
            100,
            &decrypt_key,
            &XChaCha20Poly1305Cipher,
            &Ed25519Verifier,
            Some(cache_policy),
        )
        .expect("receive should work");

        assert!(node.cache.len() <= 1);
    }

    #[test]
    fn decode_batched_payload_round_trip() {
        let items = vec![b"first".to_vec(), b"second".to_vec()];
        let encoded = serde_cbor::to_vec(&items).expect("items should encode");
        let decoded = decode_batched_payload(&encoded).expect("batched payload should decode");
        assert_eq!(decoded, items);
    }
}
