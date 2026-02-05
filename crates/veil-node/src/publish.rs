use thiserror::Error;
use veil_codec::error::CodecError;
use veil_codec::object::{
    decode_object_cbor, encode_object_cbor, object_signature_message_digest, ObjectV1, Signature,
    OBJECT_FLAG_ACK_REQUESTED, OBJECT_FLAG_BATCHED, OBJECT_FLAG_SIGNED, OBJECT_V1_VERSION,
};
use veil_codec::shard::encode_shard_cbor;
use veil_core::hash::blake3_32;
use veil_core::types::{Epoch, Namespace};
use veil_core::ObjectRoot;
use veil_core::Tag;
use veil_crypto::aead::{build_veil_aad, AeadCipher, AeadError};
use veil_crypto::signing::{Signer, SigningError};
use veil_fec::sharder::{derive_object_root, object_to_shards_with_mode_and_padding, FecError};
use veil_transport::adapter::TransportAdapter;

use crate::ack::register_pending_ack;
use crate::batch::FeedBatcher;
use crate::config::NodeRuntimeConfig;
use crate::runtime::{pump_ack_timeouts, RuntimeStats};
use crate::state::NodeState;

#[derive(Debug, Error)]
pub enum PublishError {
    #[error("codec error: {0}")]
    Codec(#[from] CodecError),
    #[error("fec error: {0}")]
    Fec(#[from] FecError),
    #[error("aead error: {0}")]
    Aead(#[from] AeadError),
    #[error("signing error: {0}")]
    Signing(#[from] SigningError),
    #[error("payload encoding error: {0}")]
    PayloadEncode(#[from] serde_cbor::Error),
    #[error("signed object requested but signer was not provided")]
    MissingSigner,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PublishResult {
    pub object_root: ObjectRoot,
    pub shards_total: usize,
    pub sent_fast: usize,
    pub sent_fallback: usize,
    pub failed_fast: usize,
    pub failed_fallback: usize,
    pub ack_tracked: bool,
}

/// Typed publish flag options to avoid manual bitfield management.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PublishOptions {
    pub signed: bool,
    pub ack_requested: bool,
    /// Additional raw flag bits (advanced/experimental).
    pub extra_flags: u16,
}

impl PublishOptions {
    pub fn signed() -> Self {
        Self {
            signed: true,
            ..Self::default()
        }
    }

    pub fn with_ack_requested(mut self, ack_requested: bool) -> Self {
        self.ack_requested = ack_requested;
        self
    }

    pub fn with_extra_flags(mut self, extra_flags: u16) -> Self {
        self.extra_flags = extra_flags;
        self
    }

    pub fn to_flags(self) -> u16 {
        let mut flags = self.extra_flags;
        if self.signed {
            flags |= OBJECT_FLAG_SIGNED;
        }
        if self.ack_requested {
            flags |= OBJECT_FLAG_ACK_REQUESTED;
        }
        flags
    }
}

/// Parameters for queue-driven publish ticks.
#[derive(Debug, Clone, Copy)]
pub struct PublishQueueTickParams<'a, PFast, PFallback> {
    pub namespace: Namespace,
    pub epoch: Epoch,
    pub tag: Tag,
    pub encrypt_key: &'a [u8; 32],
    pub now_step: u64,
    pub flags: u16,
    pub interactive_flush: bool,
    pub fast_peers: &'a [PFast],
    pub fallback_peers: &'a [PFallback],
}

/// Parameters for one publisher service tick.
pub struct PublishServiceTickParams<'a, PFast, PFallback> {
    pub batcher: &'a mut FeedBatcher,
    pub publish: PublishQueueTickParams<'a, PFast, PFallback>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PublishServiceTickResult {
    pub published: Option<PublishResult>,
    pub ack_retry_sends: usize,
}

fn derive_object_nonce(
    tag: Tag,
    namespace: Namespace,
    epoch: Epoch,
    now_step: u64,
    payload: &[u8],
) -> [u8; 24] {
    let mut preimage = Vec::with_capacity(10 + 32 + 2 + 4 + 8 + 32);
    preimage.extend_from_slice(b"objnonce-v1");
    preimage.extend_from_slice(&tag);
    preimage.extend_from_slice(&namespace.0.to_be_bytes());
    preimage.extend_from_slice(&epoch.0.to_be_bytes());
    preimage.extend_from_slice(&now_step.to_be_bytes());
    preimage.extend_from_slice(&blake3_32(payload));
    let hash = blake3_32(&preimage);
    let mut nonce = [0_u8; 24];
    nonce.copy_from_slice(&hash[..24]);
    nonce
}

#[allow(clippy::too_many_arguments)]
fn build_encoded_object(
    payload: &[u8],
    namespace: Namespace,
    epoch: Epoch,
    tag: Tag,
    encrypt_key: &[u8; 32],
    now_step: u64,
    flags: u16,
    cipher: &impl AeadCipher,
    signer: Option<&impl Signer>,
) -> Result<Vec<u8>, PublishError> {
    if (flags & OBJECT_FLAG_SIGNED) != 0 && signer.is_none() {
        return Err(PublishError::MissingSigner);
    }

    let nonce = derive_object_nonce(tag, namespace, epoch, now_step, payload);
    let aad = build_veil_aad(tag, namespace, epoch);
    let envelope = cipher.encrypt(encrypt_key, nonce, &aad, payload)?;
    let payload_root = derive_object_root(payload);

    let mut object = ObjectV1 {
        version: OBJECT_V1_VERSION,
        namespace,
        epoch,
        flags,
        tag,
        object_root: payload_root,
        sender_pubkey: None,
        signature: None,
        nonce: envelope.nonce,
        ciphertext: envelope.ciphertext,
        padding: vec![0_u8; 8],
    };

    if (flags & OBJECT_FLAG_SIGNED) != 0 {
        let signer = signer.ok_or(PublishError::MissingSigner)?;
        object.sender_pubkey = Some(signer.public_key());
        object.signature = Some(Signature([0_u8; 64]));
        let digest = object_signature_message_digest(&object)?;
        object.signature = Some(Signature(signer.sign(&digest)?));
    }

    Ok(encode_object_cbor(&object)?)
}

/// Publishes an encoded VEIL object over fast/fallback lanes and optionally
/// registers ACK-timeout retry state when `ack_requested` is set.
#[allow(clippy::too_many_arguments)]
pub fn publish_encoded_object_multi_lane<AFast: TransportAdapter, AFallback: TransportAdapter>(
    node: &mut NodeState,
    fast_adapter: &mut AFast,
    fallback_adapter: &mut AFallback,
    encoded_object: &[u8],
    fast_peers: &[AFast::Peer],
    fallback_peers: &[AFallback::Peer],
    now_step: u64,
    config: &NodeRuntimeConfig,
) -> Result<PublishResult, PublishError> {
    let object = decode_object_cbor(encoded_object)?;
    let wire_root = derive_object_root(encoded_object);
    let shards = object_to_shards_with_mode_and_padding(
        encoded_object,
        object.namespace,
        object.epoch,
        object.tag,
        wire_root,
        config.erasure_coding_mode,
        config.bucket_jitter_extra_levels,
    )?;
    let k = shards.first().map(|s| s.header.k as usize).unwrap_or(0);

    let mut shard_bytes = Vec::with_capacity(shards.len());
    for shard in &shards {
        shard_bytes.push(encode_shard_cbor(shard)?);
    }

    let fast_count = shard_bytes.len().min(k.saturating_add(2));
    let fallback_start = fast_count;
    let fallback_end = shard_bytes.len().min(fallback_start.saturating_add(2));

    let mut sent_fast = 0usize;
    let mut failed_fast = 0usize;
    for bytes in shard_bytes.iter().take(fast_count) {
        for peer in fast_peers.iter().take(config.base_fast_fanout.min(2)) {
            if fast_adapter.send(peer, bytes).is_ok() {
                sent_fast += 1;
            } else {
                failed_fast += 1;
            }
        }
    }

    let mut sent_fallback = 0usize;
    let mut failed_fallback = 0usize;
    for bytes in shard_bytes.iter().take(fallback_end).skip(fallback_start) {
        for peer in fallback_peers
            .iter()
            .take(config.base_fallback_fanout.max(1))
        {
            if fallback_adapter.send(peer, bytes).is_ok() {
                sent_fallback += 1;
            } else {
                failed_fallback += 1;
            }
        }
    }

    let mut ack_tracked = false;
    if (object.flags & OBJECT_FLAG_ACK_REQUESTED) != 0 {
        let unsent = shard_bytes[fallback_end..].to_vec();
        register_pending_ack(node, wire_root, unsent, now_step, config.ack_retry_policy());
        ack_tracked = true;
    }

    Ok(PublishResult {
        object_root: wire_root,
        shards_total: shard_bytes.len(),
        sent_fast,
        sent_fallback,
        failed_fast,
        failed_fallback,
        ack_tracked,
    })
}

/// Drains queued feed items, builds one object, and publishes it multi-lane.
///
/// Returns `Ok(None)` when the queue is empty.
#[allow(clippy::too_many_arguments)]
pub fn publish_queue_tick_multi_lane<
    AFast: TransportAdapter,
    AFallback: TransportAdapter,
    S: Signer,
>(
    node: &mut NodeState,
    fast_adapter: &mut AFast,
    fallback_adapter: &mut AFallback,
    batcher: &mut FeedBatcher,
    params: PublishQueueTickParams<'_, AFast::Peer, AFallback::Peer>,
    config: &NodeRuntimeConfig,
    cipher: &impl AeadCipher,
    signer: Option<&S>,
) -> Result<Option<PublishResult>, PublishError> {
    let items = if params.interactive_flush {
        batcher.drain_interactive()
    } else {
        batcher.drain_next_batch()
    };
    if items.is_empty() {
        return Ok(None);
    }

    let payload = serde_cbor::to_vec(&items)?;
    let mut flags = params.flags;
    if items.len() > 1 {
        flags |= OBJECT_FLAG_BATCHED;
    }
    let encoded_object = build_encoded_object(
        &payload,
        params.namespace,
        params.epoch,
        params.tag,
        params.encrypt_key,
        params.now_step,
        flags,
        cipher,
        signer,
    )?;

    let result = publish_encoded_object_multi_lane(
        node,
        fast_adapter,
        fallback_adapter,
        &encoded_object,
        params.fast_peers,
        params.fallback_peers,
        params.now_step,
        config,
    )?;
    Ok(Some(result))
}

/// Runs one publish service tick:
/// - drains and publishes one queued batch (if present)
/// - sends due ACK-timeout retries over fallback peers
///
/// This is the recommended publisher-side entrypoint for steady-state operation.
/// Call it once per scheduler step/tick after enqueuing feed payloads into the
/// batcher. The function may publish a new object, emit retry shards, or both.
pub fn publish_service_tick_multi_lane<
    AFast: TransportAdapter,
    AFallback: TransportAdapter,
    S: Signer,
>(
    node: &mut NodeState,
    fast_adapter: &mut AFast,
    fallback_adapter: &mut AFallback,
    params: PublishServiceTickParams<'_, AFast::Peer, AFallback::Peer>,
    config: &NodeRuntimeConfig,
    cipher: &impl AeadCipher,
    signer: Option<&S>,
) -> Result<PublishServiceTickResult, PublishError> {
    let publish_params = params.publish;
    let retry_peers = publish_params.fallback_peers;
    let now_step = publish_params.now_step;
    let published = publish_queue_tick_multi_lane(
        node,
        fast_adapter,
        fallback_adapter,
        params.batcher,
        publish_params,
        config,
        cipher,
        signer,
    )?;

    let mut runtime_stats = RuntimeStats::default();
    let ack_retry_sends = pump_ack_timeouts(
        node,
        fallback_adapter,
        retry_peers,
        now_step,
        config.base_fallback_fanout.max(1),
        &mut runtime_stats,
    );

    Ok(PublishServiceTickResult {
        published,
        ack_retry_sends,
    })
}

#[cfg(test)]
mod tests {
    use veil_codec::object::{
        encode_object_cbor, object_signature_message_digest, ObjectV1, Signature,
        OBJECT_FLAG_ACK_REQUESTED, OBJECT_FLAG_SIGNED, OBJECT_V1_VERSION,
    };
    use veil_core::{Epoch, Namespace};
    use veil_crypto::aead::{build_veil_aad, AeadCipher, XChaCha20Poly1305Cipher};
    use veil_crypto::signing::{Ed25519Signer, Signer};
    use veil_fec::sharder::derive_object_root;
    use veil_transport::adapter::InMemoryAdapter;

    use super::{
        publish_encoded_object_multi_lane, publish_queue_tick_multi_lane,
        publish_service_tick_multi_lane, PublishOptions, PublishQueueTickParams,
        PublishServiceTickParams,
    };
    use crate::ack::{register_pending_ack, AckRetryPolicy};
    use crate::batch::{BatchLimits, FeedBatcher};
    use crate::config::NodeRuntimeConfig;
    use crate::state::NodeState;

    fn make_encoded_object(payload: &[u8], tag: [u8; 32], key: &[u8; 32], flags: u16) -> Vec<u8> {
        let namespace = Namespace(77);
        let epoch = Epoch(88);
        let nonce = [0x33_u8; 24];
        let cipher = XChaCha20Poly1305Cipher;
        let signer = Ed25519Signer::from_secret([0x42_u8; 32]);

        let aad = build_veil_aad(tag, namespace, epoch);
        let env = cipher
            .encrypt(key, nonce, &aad, payload)
            .expect("encryption should succeed");
        let mut obj = ObjectV1 {
            version: OBJECT_V1_VERSION,
            namespace,
            epoch,
            flags,
            tag,
            object_root: derive_object_root(payload),
            sender_pubkey: Some(signer.public_key()),
            signature: Some(Signature([0_u8; 64])),
            nonce: env.nonce,
            ciphertext: env.ciphertext,
            padding: vec![0_u8; 8],
        };
        let digest = object_signature_message_digest(&obj).expect("digest should compute");
        obj.signature = Some(Signature(
            signer.sign(&digest).expect("signature should succeed"),
        ));
        encode_object_cbor(&obj).expect("encoding should succeed")
    }

    #[test]
    fn publish_tracks_pending_ack_when_requested() {
        let mut node = NodeState::default();
        let mut fast = InMemoryAdapter::default();
        let mut fallback = InMemoryAdapter::default();
        let cfg = NodeRuntimeConfig::default();
        let key = [0xAA_u8; 32];
        let tag = [0x11_u8; 32];
        let encoded = make_encoded_object(
            b"hello publish",
            tag,
            &key,
            OBJECT_FLAG_SIGNED | OBJECT_FLAG_ACK_REQUESTED,
        );
        let peers = vec!["peer-a".to_string(), "peer-b".to_string()];

        let out = publish_encoded_object_multi_lane(
            &mut node,
            &mut fast,
            &mut fallback,
            &encoded,
            &peers,
            &peers,
            10,
            &cfg,
        )
        .expect("publish should succeed");

        assert!(out.sent_fast > 0);
        assert!(out.sent_fallback > 0);
        assert!(out.ack_tracked);
        assert!(node.pending_acks.contains_key(&out.object_root));
    }

    #[test]
    fn publish_skips_ack_tracking_without_flag() {
        let mut node = NodeState::default();
        let mut fast = InMemoryAdapter::default();
        let mut fallback = InMemoryAdapter::default();
        let cfg = NodeRuntimeConfig::default();
        let key = [0xBB_u8; 32];
        let tag = [0x22_u8; 32];
        let encoded = make_encoded_object(b"no ack", tag, &key, OBJECT_FLAG_SIGNED);
        let peers = vec!["peer-a".to_string(), "peer-b".to_string()];

        let out = publish_encoded_object_multi_lane(
            &mut node,
            &mut fast,
            &mut fallback,
            &encoded,
            &peers,
            &peers,
            10,
            &cfg,
        )
        .expect("publish should succeed");

        assert!(!out.ack_tracked);
        assert!(node.pending_acks.is_empty());
    }

    #[test]
    fn publish_queue_tick_drains_batch_and_publishes() {
        let mut node = NodeState::default();
        let mut fast = InMemoryAdapter::default();
        let mut fallback = InMemoryAdapter::default();
        let cfg = NodeRuntimeConfig::default();
        let mut batcher = FeedBatcher::with_limits(BatchLimits {
            target_batch_size: 64,
            max_object_size: 512,
        });
        batcher.enqueue(vec![1_u8; 40]);
        batcher.enqueue(vec![2_u8; 40]);

        let peers = vec!["peer-a".to_string(), "peer-b".to_string()];
        let signer = Ed25519Signer::from_secret([0x55; 32]);
        let out = publish_queue_tick_multi_lane(
            &mut node,
            &mut fast,
            &mut fallback,
            &mut batcher,
            PublishQueueTickParams {
                namespace: Namespace(7),
                epoch: Epoch(9),
                tag: [0x33; 32],
                encrypt_key: &[0xAA; 32],
                now_step: 10,
                flags: OBJECT_FLAG_SIGNED,
                interactive_flush: false,
                fast_peers: &peers,
                fallback_peers: &peers,
            },
            &cfg,
            &XChaCha20Poly1305Cipher,
            Some(&signer),
        )
        .expect("queue publish should succeed")
        .expect("queue should yield one publish result");

        assert!(out.sent_fast > 0);
        assert_eq!(batcher.len(), 0);
    }

    #[test]
    fn publish_queue_tick_requires_signer_when_signed_flag_set() {
        let mut node = NodeState::default();
        let mut fast = InMemoryAdapter::default();
        let mut fallback = InMemoryAdapter::default();
        let cfg = NodeRuntimeConfig::default();
        let mut batcher = FeedBatcher::default();
        batcher.enqueue(vec![9_u8; 8]);
        let peers = vec!["peer-a".to_string()];

        let err = publish_queue_tick_multi_lane::<InMemoryAdapter, InMemoryAdapter, Ed25519Signer>(
            &mut node,
            &mut fast,
            &mut fallback,
            &mut batcher,
            PublishQueueTickParams {
                namespace: Namespace(1),
                epoch: Epoch(1),
                tag: [0x11; 32],
                encrypt_key: &[0xAB; 32],
                now_step: 1,
                flags: OBJECT_FLAG_SIGNED,
                interactive_flush: true,
                fast_peers: &peers,
                fallback_peers: &peers,
            },
            &cfg,
            &XChaCha20Poly1305Cipher,
            None,
        )
        .expect_err("missing signer should fail");

        assert!(err.to_string().contains("signer"));
    }

    #[test]
    fn publish_service_tick_sends_due_ack_retries_without_new_batch() {
        let mut node = NodeState::default();
        register_pending_ack(
            &mut node,
            [0x55; 32],
            vec![vec![1, 2, 3], vec![4, 5, 6]],
            0,
            AckRetryPolicy {
                initial_timeout_steps: 1,
                retry_batch_size: 2,
                backoff_step: 2,
                max_retries: 3,
            },
        );

        let mut fast = InMemoryAdapter::default();
        let mut fallback = InMemoryAdapter::default();
        let mut batcher = FeedBatcher::default();
        let peers = vec!["peer-a".to_string(), "peer-b".to_string()];
        let cfg = NodeRuntimeConfig::default();
        let signer = Ed25519Signer::from_secret([0x42_u8; 32]);

        let out = publish_service_tick_multi_lane(
            &mut node,
            &mut fast,
            &mut fallback,
            PublishServiceTickParams {
                batcher: &mut batcher,
                publish: PublishQueueTickParams {
                    namespace: Namespace(1),
                    epoch: Epoch(1),
                    tag: [0x11; 32],
                    encrypt_key: &[0xAA; 32],
                    now_step: 1,
                    flags: OBJECT_FLAG_SIGNED,
                    interactive_flush: false,
                    fast_peers: &peers,
                    fallback_peers: &peers,
                },
            },
            &cfg,
            &XChaCha20Poly1305Cipher,
            Some(&signer),
        )
        .expect("service tick should succeed");

        assert!(out.published.is_none());
        assert!(out.ack_retry_sends > 0);
    }

    #[test]
    fn publish_options_build_expected_flag_bits() {
        let flags = PublishOptions::signed()
            .with_ack_requested(true)
            .with_extra_flags(0x0008)
            .to_flags();

        assert!((flags & OBJECT_FLAG_SIGNED) != 0);
        assert!((flags & OBJECT_FLAG_ACK_REQUESTED) != 0);
        assert!((flags & 0x0008) != 0);
    }
}
