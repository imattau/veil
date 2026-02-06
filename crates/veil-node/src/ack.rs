use thiserror::Error;
use veil_codec::error::CodecError;
use veil_codec::object::{encode_object_cbor, ObjectV1, OBJECT_V1_VERSION};
use veil_codec::shard::encode_shard_cbor;
use veil_core::hash::blake3_32;
use veil_core::ObjectRoot;
use veil_core::{Epoch, Namespace, Tag};
use veil_crypto::aead::{build_veil_aad, AeadCipher, AeadError};
use veil_fec::profile::ErasureCodingMode;
use veil_fec::sharder::{derive_object_root, object_to_shards_with_mode_and_padding, FecError};

use crate::state::{NodeState, PendingAck};

const ACK_PAYLOAD_MAGIC: &[u8] = b"VEIL_ACK_V1";

#[derive(Debug, Error)]
pub enum AckBuildError {
    #[error("codec error: {0}")]
    Codec(#[from] CodecError),
    #[error("aead error: {0}")]
    Aead(#[from] AeadError),
    #[error("fec error: {0}")]
    Fec(#[from] FecError),
}

/// Retry policy for ACK-timeout escalation batches.
#[derive(Debug, Clone, Copy)]
pub struct AckRetryPolicy {
    /// Steps to wait before the first retry batch is sent.
    pub initial_timeout_steps: u64,
    /// Number of unsent shard payloads to include in each retry.
    pub retry_batch_size: usize,
    /// Step delay between subsequent retry attempts.
    pub backoff_step: u64,
    /// Hard cap on retry attempts for this pending ACK entry.
    pub max_retries: u32,
}

/// Registers pending ACK state for an outbound object.
pub fn register_pending_ack(
    node: &mut NodeState,
    object_root: ObjectRoot,
    unsent_shards: Vec<Vec<u8>>,
    now_step: u64,
    retry_policy: AckRetryPolicy,
) {
    node.pending_acks.insert(
        object_root,
        PendingAck {
            unsent_shards,
            next_retry_step: now_step + retry_policy.initial_timeout_steps,
            retries: 0,
            max_retries: retry_policy.max_retries,
            retry_batch_size: retry_policy.retry_batch_size,
            backoff_step: retry_policy.backoff_step,
        },
    );
}

/// Marks an ACK as received for `object_root`, clearing pending retry state.
pub fn ack_received(node: &mut NodeState, object_root: ObjectRoot) -> bool {
    node.pending_acks.remove(&object_root).is_some()
}

/// Encodes an ACK payload carrying the acknowledged wire object root.
pub fn encode_ack_payload(object_root: ObjectRoot) -> Vec<u8> {
    let mut payload = Vec::with_capacity(ACK_PAYLOAD_MAGIC.len() + object_root.len());
    payload.extend_from_slice(ACK_PAYLOAD_MAGIC);
    payload.extend_from_slice(&object_root);
    payload
}

/// Decodes an ACK payload into its acknowledged wire object root.
pub fn decode_ack_payload(payload: &[u8]) -> Option<ObjectRoot> {
    if payload.len() != ACK_PAYLOAD_MAGIC.len() + 32 {
        return None;
    }
    if &payload[..ACK_PAYLOAD_MAGIC.len()] != ACK_PAYLOAD_MAGIC {
        return None;
    }
    let mut root = [0_u8; 32];
    root.copy_from_slice(&payload[ACK_PAYLOAD_MAGIC.len()..]);
    Some(root)
}

/// Builds ACK object shards (already encoded as shard CBOR bytes) for sending.
pub fn build_ack_shard_bytes(
    acked_object_root: ObjectRoot,
    tag: Tag,
    namespace: Namespace,
    epoch: Epoch,
    encrypt_key: &[u8; 32],
    cipher: &impl AeadCipher,
) -> Result<Vec<Vec<u8>>, AckBuildError> {
    build_ack_shard_bytes_with_mode(
        acked_object_root,
        tag,
        namespace,
        epoch,
        encrypt_key,
        cipher,
        ErasureCodingMode::HardenedNonSystematic,
    )
}

/// Builds ACK object shards (already encoded as shard CBOR bytes) for sending.
pub fn build_ack_shard_bytes_with_mode(
    acked_object_root: ObjectRoot,
    tag: Tag,
    namespace: Namespace,
    epoch: Epoch,
    encrypt_key: &[u8; 32],
    cipher: &impl AeadCipher,
    mode: ErasureCodingMode,
) -> Result<Vec<Vec<u8>>, AckBuildError> {
    build_ack_shard_bytes_with_mode_and_padding(
        acked_object_root,
        tag,
        namespace,
        epoch,
        encrypt_key,
        cipher,
        mode,
        0,
    )
}

/// Builds ACK object shards with explicit coding mode and bucket jitter.
#[allow(clippy::too_many_arguments)]
pub fn build_ack_shard_bytes_with_mode_and_padding(
    acked_object_root: ObjectRoot,
    tag: Tag,
    namespace: Namespace,
    epoch: Epoch,
    encrypt_key: &[u8; 32],
    cipher: &impl AeadCipher,
    mode: ErasureCodingMode,
    bucket_jitter_extra_levels: usize,
) -> Result<Vec<Vec<u8>>, AckBuildError> {
    let payload = encode_ack_payload(acked_object_root);
    let aad = build_veil_aad(tag, namespace, epoch);

    let mut nonce_seed = Vec::with_capacity(ACK_PAYLOAD_MAGIC.len() + acked_object_root.len());
    nonce_seed.extend_from_slice(ACK_PAYLOAD_MAGIC);
    nonce_seed.extend_from_slice(&acked_object_root);
    let nonce_hash = blake3_32(&nonce_seed);
    let mut nonce = [0_u8; 24];
    nonce.copy_from_slice(&nonce_hash[..24]);

    let envelope = cipher.encrypt(encrypt_key, nonce, &aad, &payload)?;
    let object = ObjectV1 {
        version: OBJECT_V1_VERSION,
        namespace,
        epoch,
        flags: 0,
        tag,
        object_root: derive_object_root(&payload),
        sender_pubkey: None,
        signature: None,
        nonce: envelope.nonce,
        ciphertext: envelope.ciphertext,
        padding: vec![0_u8; 8],
    };
    let encoded_object = encode_object_cbor(&object)?;
    let wire_root = derive_object_root(&encoded_object);
    let shards = object_to_shards_with_mode_and_padding(
        &encoded_object,
        namespace,
        epoch,
        tag,
        wire_root,
        mode,
        bucket_jitter_extra_levels,
    )?;

    let mut out = Vec::with_capacity(shards.len());
    for shard in &shards {
        out.push(encode_shard_cbor(shard)?);
    }
    Ok(out)
}

/// Returns the next due ACK-timeout retry batch, if any.
///
/// The returned batch contains shard bytes ready to send over a fallback lane.
pub fn next_ack_escalation_batch(
    node: &mut NodeState,
    now_step: u64,
) -> Option<(ObjectRoot, Vec<Vec<u8>>)> {
    let due_root = node
        .pending_acks
        .iter()
        .find_map(|(root, pending)| (pending.next_retry_step <= now_step).then_some(*root))?;

    let pending = node.pending_acks.get_mut(&due_root)?;
    if pending.unsent_shards.is_empty() || pending.retries >= pending.max_retries {
        node.pending_acks.remove(&due_root);
        return None;
    }

    let take = pending.retry_batch_size.min(pending.unsent_shards.len());
    let batch: Vec<Vec<u8>> = pending.unsent_shards.drain(0..take).collect();
    pending.retries += 1;
    pending.next_retry_step = now_step + pending.backoff_step;

    if pending.unsent_shards.is_empty() || pending.retries >= pending.max_retries {
        node.pending_acks.remove(&due_root);
    }

    Some((due_root, batch))
}

#[cfg(test)]
mod tests {
    use super::{
        ack_received, build_ack_shard_bytes, decode_ack_payload, encode_ack_payload,
        next_ack_escalation_batch, register_pending_ack, AckRetryPolicy,
    };
    use crate::state::NodeState;
    use veil_core::{Epoch, Namespace};
    use veil_crypto::aead::XChaCha20Poly1305Cipher;

    #[test]
    fn no_escalation_before_timeout() {
        let mut node = NodeState::default();
        register_pending_ack(
            &mut node,
            [0x11; 32],
            vec![vec![1], vec![2]],
            10,
            AckRetryPolicy {
                initial_timeout_steps: 3,
                retry_batch_size: 1,
                backoff_step: 2,
                max_retries: 3,
            },
        );
        assert!(next_ack_escalation_batch(&mut node, 12).is_none());
    }

    #[test]
    fn escalation_drains_retry_batches_and_clears_when_done() {
        let mut node = NodeState::default();
        let root = [0x22; 32];
        register_pending_ack(
            &mut node,
            root,
            vec![vec![1], vec![2], vec![3]],
            5,
            AckRetryPolicy {
                initial_timeout_steps: 1,
                retry_batch_size: 2,
                backoff_step: 2,
                max_retries: 3,
            },
        );

        let (_, batch1) =
            next_ack_escalation_batch(&mut node, 6).expect("batch should be available");
        assert_eq!(batch1.len(), 2);
        assert!(node.pending_acks.contains_key(&root));

        let (_, batch2) =
            next_ack_escalation_batch(&mut node, 8).expect("second batch should be available");
        assert_eq!(batch2.len(), 1);
        assert!(!node.pending_acks.contains_key(&root));
    }

    #[test]
    fn ack_received_clears_pending_entry() {
        let mut node = NodeState::default();
        let root = [0x33; 32];
        register_pending_ack(
            &mut node,
            root,
            vec![vec![7]],
            0,
            AckRetryPolicy {
                initial_timeout_steps: 1,
                retry_batch_size: 1,
                backoff_step: 2,
                max_retries: 1,
            },
        );

        assert!(ack_received(&mut node, root));
        assert!(!ack_received(&mut node, root));
    }

    #[test]
    fn ack_payload_round_trip() {
        let root = [0x99; 32];
        let payload = encode_ack_payload(root);
        let parsed = decode_ack_payload(&payload).expect("ack payload should parse");
        assert_eq!(parsed, root);
    }

    #[test]
    fn ack_payload_rejects_invalid_magic() {
        let mut payload = encode_ack_payload([0xAB; 32]);
        payload[0] ^= 0x01;
        assert!(decode_ack_payload(&payload).is_none());
    }

    #[test]
    fn build_ack_shards_produces_non_empty_output() {
        let shards = build_ack_shard_bytes(
            [0x55; 32],
            [0x66; 32],
            Namespace(9),
            Epoch(10),
            &[0x77; 32],
            &XChaCha20Poly1305Cipher,
        )
        .expect("ack shards should be built");
        assert!(!shards.is_empty());
    }
}
