use veil_codec::object::OBJECT_FLAG_ACK_REQUESTED;
use veil_codec::shard::decode_shard_cbor;
use veil_crypto::aead::AeadCipher;
use veil_crypto::signing::Verifier;
use veil_transport::adapter::TransportAdapter;

use crate::ack::{
    ack_received, build_ack_shard_bytes, decode_ack_payload, next_ack_escalation_batch,
};
use crate::config::NodeRuntimeConfig;
use crate::policy::{TrustTier, WotPolicy};
use crate::receive::{receive_shard_with_policy, ReceiveCachePolicy, ReceiveError, ReceiveEvent};
use crate::state::NodeState;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeStats {
    /// Total inbound byte payloads polled from transports.
    pub inbound_messages: usize,
    /// Inbound payloads successfully parsed as `ShardV1`.
    pub parsed_shards: usize,
    /// Outbound sends performed by forwarding / ACK pumps.
    pub forwarded_messages: usize,
    /// Inbound messages ignored as non-shards, duplicates, or unsubscribed.
    pub ignored_messages: usize,
    /// Inbound shard messages ignored due to duplicate shard id.
    pub duplicate_messages: usize,
    /// Delivered reconstructed objects.
    pub delivered_messages: usize,
    /// ACK payload objects recognized and matched to pending ACK state.
    pub ack_messages: usize,
}

/// Parameters for a single-lane `pump_once` call.
pub struct PumpParams<'a, P> {
    pub peers: &'a [P],
    pub now_step: u64,
    pub ttl_steps: u64,
    pub fanout: usize,
    pub policy_hooks: RuntimePolicyHooks<'a, P>,
    pub decrypt_key: &'a [u8; 32],
    pub stats: &'a mut RuntimeStats,
}

/// Configuration-driven wrapper parameters for `pump_once_with_config`.
pub struct ConfigPumpParams<'a, P> {
    pub peers: &'a [P],
    pub now_step: u64,
    pub decrypt_key: &'a [u8; 32],
    pub config: &'a NodeRuntimeConfig,
    pub stats: &'a mut RuntimeStats,
}

/// Per-lane peer set and fanout for multi-lane pumping.
pub struct LaneForwardParams<'a, P> {
    pub peers: &'a [P],
    pub fanout: usize,
}

/// Parameters for `pump_multi_lane_once`.
pub struct MultiLanePumpParams<'a, P> {
    pub fast_lane: LaneForwardParams<'a, P>,
    pub fallback_lane: LaneForwardParams<'a, P>,
    pub fallback_redundancy_fanout: usize,
    pub now_step: u64,
    pub ttl_steps: u64,
    pub policy_hooks: RuntimePolicyHooks<'a, P>,
    pub decrypt_key: &'a [u8; 32],
    pub stats: &'a mut RuntimeStats,
}

/// Configuration-driven wrapper parameters for `pump_multi_lane_once_with_config`.
pub struct ConfigMultiLanePumpParams<'a, P> {
    pub fast_peers: &'a [P],
    pub fallback_peers: &'a [P],
    pub now_step: u64,
    pub decrypt_key: &'a [u8; 32],
    pub config: &'a NodeRuntimeConfig,
    pub stats: &'a mut RuntimeStats,
}

/// Optional callback used to adjust fanout per inbound peer.
pub type PeerFanoutFn<'a, P> = dyn Fn(&P, u64, usize) -> usize + 'a;
/// Optional callback used to classify peer trust tier for cache policy.
pub type PeerTierFn<'a, P> = dyn Fn(&P, u64) -> TrustTier + 'a;

#[derive(Clone, Copy)]
pub struct RuntimePolicyHooks<'a, P> {
    pub fanout_for_peer: Option<&'a PeerFanoutFn<'a, P>>,
    pub classify_peer_tier: Option<&'a PeerTierFn<'a, P>>,
    pub max_cache_shards: usize,
    pub wot_policy: Option<&'a dyn WotPolicy>,
}

impl<'a, P> Default for RuntimePolicyHooks<'a, P> {
    fn default() -> Self {
        Self {
            fanout_for_peer: None,
            classify_peer_tier: None,
            max_cache_shards: usize::MAX,
            wot_policy: None,
        }
    }
}

struct InboundProcessParams<'a, P> {
    from_peer: &'a P,
    bytes: &'a [u8],
    peers: &'a [P],
    fanout: usize,
    now_step: u64,
    ttl_steps: u64,
    decrypt_key: &'a [u8; 32],
    cache_policy: Option<ReceiveCachePolicy<'a>>,
    stats: &'a mut RuntimeStats,
}

fn is_forwardable(event: &ReceiveEvent) -> bool {
    matches!(
        event,
        ReceiveEvent::Buffered { .. } | ReceiveEvent::Delivered { .. }
    )
}

fn process_inbound<A: TransportAdapter>(
    node: &mut NodeState,
    adapter: &mut A,
    params: InboundProcessParams<'_, A::Peer>,
    cipher: &impl AeadCipher,
    verifier: &impl Verifier,
) -> Result<ReceiveEvent, ReceiveError> {
    let InboundProcessParams {
        from_peer,
        bytes,
        peers,
        fanout,
        now_step,
        ttl_steps,
        decrypt_key,
        cache_policy,
        stats,
    } = params;

    stats.inbound_messages += 1;

    let shard = match decode_shard_cbor(bytes) {
        Ok(shard) => shard,
        Err(_) => {
            stats.ignored_messages += 1;
            return Ok(ReceiveEvent::IgnoredNotSubscribed);
        }
    };
    stats.parsed_shards += 1;

    let event = receive_shard_with_policy(
        node,
        &shard,
        now_step,
        ttl_steps,
        decrypt_key,
        cipher,
        verifier,
        cache_policy,
    )?;

    if is_forwardable(&event) {
        for peer in peers
            .iter()
            .filter(|peer| **peer != *from_peer)
            .take(fanout)
        {
            if adapter.send(peer, bytes).is_ok() {
                stats.forwarded_messages += 1;
            }
        }
    }

    if let ReceiveEvent::Delivered {
        object_root,
        payload,
        namespace,
        epoch,
        tag,
        flags,
    } = &event
    {
        stats.delivered_messages += 1;
        if let Some(acked_root) = decode_ack_payload(payload) {
            if ack_received(node, acked_root) {
                stats.ack_messages += 1;
            }
        }
        if (flags & OBJECT_FLAG_ACK_REQUESTED) != 0 {
            if let Ok(ack_shards) =
                build_ack_shard_bytes(*object_root, *tag, *namespace, *epoch, decrypt_key, cipher)
            {
                for ack_shard in &ack_shards {
                    if adapter.send(from_peer, ack_shard).is_ok() {
                        stats.forwarded_messages += 1;
                    }
                }
            }
        }
    }
    if matches!(event, ReceiveEvent::IgnoredDuplicate) {
        stats.duplicate_messages += 1;
        stats.ignored_messages += 1;
    }
    if matches!(event, ReceiveEvent::IgnoredNotSubscribed) {
        stats.ignored_messages += 1;
    }

    Ok(event)
}

pub fn pump_once<A: TransportAdapter>(
    node: &mut NodeState,
    adapter: &mut A,
    params: PumpParams<'_, A::Peer>,
    cipher: &impl AeadCipher,
    verifier: &impl Verifier,
) -> Result<Option<ReceiveEvent>, ReceiveError> {
    let PumpParams {
        peers,
        now_step,
        ttl_steps,
        fanout,
        policy_hooks,
        decrypt_key,
        stats,
    } = params;

    let Some((from_peer, bytes)) = adapter.recv() else {
        return Ok(None);
    };
    let effective_fanout = policy_hooks
        .fanout_for_peer
        .map(|f| f(&from_peer, now_step, fanout))
        .unwrap_or(fanout);
    let cache_policy = match (policy_hooks.classify_peer_tier, policy_hooks.wot_policy) {
        (Some(classify_peer_tier), Some(wot_policy)) => Some(ReceiveCachePolicy {
            tier: classify_peer_tier(&from_peer, now_step),
            max_cache_shards: policy_hooks.max_cache_shards,
            wot_policy,
        }),
        _ => None,
    };
    let event = process_inbound(
        node,
        adapter,
        InboundProcessParams {
            from_peer: &from_peer,
            bytes: &bytes,
            peers,
            fanout: effective_fanout,
            now_step,
            ttl_steps,
            decrypt_key,
            cache_policy,
            stats,
        },
        cipher,
        verifier,
    )?;
    Ok(Some(event))
}

/// Convenience wrapper around `pump_once` using `NodeRuntimeConfig`.
pub fn pump_once_with_config<A>(
    node: &mut NodeState,
    adapter: &mut A,
    params: ConfigPumpParams<'_, A::Peer>,
    cipher: &impl AeadCipher,
    verifier: &impl Verifier,
) -> Result<Option<ReceiveEvent>, ReceiveError>
where
    A: TransportAdapter,
    A::Peer: ToString,
{
    let ConfigPumpParams {
        peers,
        now_step,
        decrypt_key,
        config,
        stats,
    } = params;

    let fanout_fn = |peer: &A::Peer, step: u64, base: usize| {
        config.fanout_for_peer(&peer.to_string(), step, base)
    };
    let tier_fn = |peer: &A::Peer, step: u64| config.classify_peer_tier(&peer.to_string(), step);

    pump_once(
        node,
        adapter,
        PumpParams {
            peers,
            now_step,
            ttl_steps: config.ttl_steps,
            fanout: config.base_fast_fanout,
            policy_hooks: RuntimePolicyHooks {
                fanout_for_peer: Some(&fanout_fn),
                classify_peer_tier: Some(&tier_fn),
                max_cache_shards: config.max_cache_shards,
                wot_policy: Some(&config.wot_policy),
            },
            decrypt_key,
            stats,
        },
        cipher,
        verifier,
    )
}

/// Runs one multi-lane runtime step:
/// reads from fast lane first, then fallback, and forwards as configured.
pub fn pump_multi_lane_once<A: TransportAdapter>(
    node: &mut NodeState,
    fast_adapter: &mut A,
    fallback_adapter: &mut A,
    params: MultiLanePumpParams<'_, A::Peer>,
    cipher: &impl AeadCipher,
    verifier: &impl Verifier,
) -> Result<Option<ReceiveEvent>, ReceiveError> {
    let MultiLanePumpParams {
        fast_lane,
        fallback_lane,
        fallback_redundancy_fanout,
        now_step,
        ttl_steps,
        policy_hooks,
        decrypt_key,
        stats,
    } = params;

    if let Some((from_peer, bytes)) = fast_adapter.recv() {
        let effective_fast_fanout = policy_hooks
            .fanout_for_peer
            .map(|f| f(&from_peer, now_step, fast_lane.fanout))
            .unwrap_or(fast_lane.fanout);
        let cache_policy = match (policy_hooks.classify_peer_tier, policy_hooks.wot_policy) {
            (Some(classify_peer_tier), Some(wot_policy)) => Some(ReceiveCachePolicy {
                tier: classify_peer_tier(&from_peer, now_step),
                max_cache_shards: policy_hooks.max_cache_shards,
                wot_policy,
            }),
            _ => None,
        };
        let event = process_inbound(
            node,
            fast_adapter,
            InboundProcessParams {
                from_peer: &from_peer,
                bytes: &bytes,
                peers: fast_lane.peers,
                fanout: effective_fast_fanout,
                now_step,
                ttl_steps,
                decrypt_key,
                cache_policy,
                stats,
            },
            cipher,
            verifier,
        )?;

        let effective_redundancy = policy_hooks
            .fanout_for_peer
            .map(|f| f(&from_peer, now_step, fallback_redundancy_fanout))
            .unwrap_or(fallback_redundancy_fanout);

        if is_forwardable(&event) && effective_redundancy > 0 {
            for peer in fallback_lane
                .peers
                .iter()
                .filter(|peer| **peer != from_peer)
                .take(effective_redundancy)
            {
                if fallback_adapter.send(peer, &bytes).is_ok() {
                    stats.forwarded_messages += 1;
                }
            }
        }
        return Ok(Some(event));
    }

    if let Some((from_peer, bytes)) = fallback_adapter.recv() {
        let effective_fallback_fanout = policy_hooks
            .fanout_for_peer
            .map(|f| f(&from_peer, now_step, fallback_lane.fanout))
            .unwrap_or(fallback_lane.fanout);
        let cache_policy = match (policy_hooks.classify_peer_tier, policy_hooks.wot_policy) {
            (Some(classify_peer_tier), Some(wot_policy)) => Some(ReceiveCachePolicy {
                tier: classify_peer_tier(&from_peer, now_step),
                max_cache_shards: policy_hooks.max_cache_shards,
                wot_policy,
            }),
            _ => None,
        };
        let event = process_inbound(
            node,
            fallback_adapter,
            InboundProcessParams {
                from_peer: &from_peer,
                bytes: &bytes,
                peers: fallback_lane.peers,
                fanout: effective_fallback_fanout,
                now_step,
                ttl_steps,
                decrypt_key,
                cache_policy,
                stats,
            },
            cipher,
            verifier,
        )?;
        return Ok(Some(event));
    }

    Ok(None)
}

/// Convenience wrapper around `pump_multi_lane_once` using `NodeRuntimeConfig`.
pub fn pump_multi_lane_once_with_config<A>(
    node: &mut NodeState,
    fast_adapter: &mut A,
    fallback_adapter: &mut A,
    params: ConfigMultiLanePumpParams<'_, A::Peer>,
    cipher: &impl AeadCipher,
    verifier: &impl Verifier,
) -> Result<Option<ReceiveEvent>, ReceiveError>
where
    A: TransportAdapter,
    A::Peer: ToString,
{
    let ConfigMultiLanePumpParams {
        fast_peers,
        fallback_peers,
        now_step,
        decrypt_key,
        config,
        stats,
    } = params;

    let fanout_fn = |peer: &A::Peer, step: u64, base: usize| {
        config.fanout_for_peer(&peer.to_string(), step, base)
    };
    let tier_fn = |peer: &A::Peer, step: u64| config.classify_peer_tier(&peer.to_string(), step);

    pump_multi_lane_once(
        node,
        fast_adapter,
        fallback_adapter,
        MultiLanePumpParams {
            fast_lane: LaneForwardParams {
                peers: fast_peers,
                fanout: config.base_fast_fanout,
            },
            fallback_lane: LaneForwardParams {
                peers: fallback_peers,
                fanout: config.base_fallback_fanout,
            },
            fallback_redundancy_fanout: config.fallback_redundancy_fanout,
            now_step,
            ttl_steps: config.ttl_steps,
            policy_hooks: RuntimePolicyHooks {
                fanout_for_peer: Some(&fanout_fn),
                classify_peer_tier: Some(&tier_fn),
                max_cache_shards: config.max_cache_shards,
                wot_policy: Some(&config.wot_policy),
            },
            decrypt_key,
            stats,
        },
        cipher,
        verifier,
    )
}

/// Runs one multi-lane runtime tick and then processes due ACK-timeout retries.
pub fn pump_multi_lane_tick_with_config<A>(
    node: &mut NodeState,
    fast_adapter: &mut A,
    fallback_adapter: &mut A,
    params: ConfigMultiLanePumpParams<'_, A::Peer>,
    cipher: &impl AeadCipher,
    verifier: &impl Verifier,
) -> Result<Option<ReceiveEvent>, ReceiveError>
where
    A: TransportAdapter,
    A::Peer: ToString,
{
    let ConfigMultiLanePumpParams {
        fast_peers,
        fallback_peers,
        now_step,
        decrypt_key,
        config,
        stats,
    } = params;

    let event = pump_multi_lane_once_with_config(
        node,
        fast_adapter,
        fallback_adapter,
        ConfigMultiLanePumpParams {
            fast_peers,
            fallback_peers,
            now_step,
            decrypt_key,
            config,
            stats,
        },
        cipher,
        verifier,
    )?;

    let _ = pump_ack_timeouts(
        node,
        fallback_adapter,
        fallback_peers,
        now_step,
        config.base_fallback_fanout.max(1),
        stats,
    );

    Ok(event)
}

/// Sends due ACK-timeout retry batches over the fallback lane.
///
/// Returns the number of successful send operations.
pub fn pump_ack_timeouts<A: TransportAdapter>(
    node: &mut NodeState,
    fallback_adapter: &mut A,
    fallback_peers: &[A::Peer],
    now_step: u64,
    fallback_fanout: usize,
    stats: &mut RuntimeStats,
) -> usize {
    let mut sent = 0;
    while let Some((_root, batch)) = next_ack_escalation_batch(node, now_step) {
        for shard_bytes in batch {
            for peer in fallback_peers.iter().take(fallback_fanout) {
                if fallback_adapter.send(peer, &shard_bytes).is_ok() {
                    stats.forwarded_messages += 1;
                    sent += 1;
                }
            }
        }
    }
    sent
}

#[cfg(test)]
mod tests {
    use veil_codec::object::{
        encode_object_cbor, object_signature_message_digest, ObjectV1, Signature,
        OBJECT_FLAG_ACK_REQUESTED, OBJECT_FLAG_SIGNED, OBJECT_V1_VERSION,
    };
    use veil_codec::shard::encode_shard_cbor;
    use veil_core::hash::blake3_32;
    use veil_core::{Epoch, Namespace};
    use veil_crypto::aead::{build_veil_aad, AeadCipher, XChaCha20Poly1305Cipher};
    use veil_crypto::signing::{Ed25519Signer, Ed25519Verifier, Signer};
    use veil_fec::sharder::{derive_object_root, object_to_shards};
    use veil_transport::adapter::InMemoryAdapter;

    use super::{
        pump_ack_timeouts, pump_multi_lane_once, pump_multi_lane_once_with_config,
        pump_multi_lane_tick_with_config, pump_once, pump_once_with_config,
        ConfigMultiLanePumpParams, ConfigPumpParams, LaneForwardParams, MultiLanePumpParams,
        PumpParams, RuntimePolicyHooks, RuntimeStats,
    };
    use crate::ack::{encode_ack_payload, register_pending_ack, AckRetryPolicy};
    use crate::config::NodeRuntimeConfig;
    use crate::state::NodeState;

    fn make_encoded_object_with_flags(
        payload: &[u8],
        tag: [u8; 32],
        key: &[u8; 32],
        flags: u16,
    ) -> Vec<u8> {
        let namespace = Namespace(7);
        let epoch = Epoch(42);
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

    fn make_encoded_object(payload: &[u8], tag: [u8; 32], key: &[u8; 32]) -> Vec<u8> {
        make_encoded_object_with_flags(payload, tag, key, OBJECT_FLAG_SIGNED)
    }

    #[test]
    fn runtime_loop_parses_forwards_and_delivers() {
        let mut node = NodeState::default();
        let tag = [0x11_u8; 32];
        node.subscriptions.insert(tag);

        let key = [0xA5_u8; 32];
        let payload = b"runtime delivery test";
        let encoded_object = make_encoded_object(payload, tag, &key);
        let root = blake3_32(&encoded_object);
        let shards = object_to_shards(&encoded_object, Namespace(7), Epoch(42), tag, root)
            .expect("sharding should succeed");
        let k = shards[0].header.k as usize;

        let mut adapter = InMemoryAdapter::default();
        for shard in shards.iter().take(k) {
            let bytes = encode_shard_cbor(shard).expect("shard should encode");
            adapter.enqueue_inbound("sender", bytes);
        }

        let peers = vec![
            "sender".to_string(),
            "peer-a".to_string(),
            "peer-b".to_string(),
        ];
        let mut stats = RuntimeStats::default();
        let mut delivered = false;
        for step in 0..k {
            let event = pump_once(
                &mut node,
                &mut adapter,
                PumpParams {
                    peers: &peers,
                    now_step: step as u64,
                    ttl_steps: 100,
                    fanout: 2,
                    policy_hooks: RuntimePolicyHooks::default(),
                    decrypt_key: &key,
                    stats: &mut stats,
                },
                &XChaCha20Poly1305Cipher,
                &Ed25519Verifier,
            )
            .expect("pump should succeed");
            if let Some(crate::receive::ReceiveEvent::Delivered { payload: got, .. }) = event {
                assert_eq!(got, payload.to_vec());
                delivered = true;
            }
        }
        assert!(delivered, "expected delivery after receiving k shards");
        assert!(
            stats.forwarded_messages > 0,
            "expected forwarding to happen"
        );
    }

    #[test]
    fn runtime_ignores_non_shard_payloads() {
        let mut node = NodeState::default();
        let mut adapter = InMemoryAdapter::default();
        adapter.enqueue_inbound("sender", vec![0xDE, 0xAD, 0xBE, 0xEF]);
        let mut stats = RuntimeStats::default();

        let event = pump_once(
            &mut node,
            &mut adapter,
            PumpParams {
                peers: &["sender".to_string()],
                now_step: 0,
                ttl_steps: 50,
                fanout: 1,
                policy_hooks: RuntimePolicyHooks::default(),
                decrypt_key: &[0x11_u8; 32],
                stats: &mut stats,
            },
            &XChaCha20Poly1305Cipher,
            &Ed25519Verifier,
        )
        .expect("pump should not fail");

        assert!(matches!(
            event,
            Some(crate::receive::ReceiveEvent::IgnoredNotSubscribed)
        ));
        assert_eq!(stats.parsed_shards, 0);
        assert_eq!(stats.ignored_messages, 1);
    }

    #[test]
    fn multi_lane_pump_fallback_ingest_still_delivers() {
        let mut node = NodeState::default();
        let tag = [0x21_u8; 32];
        node.subscriptions.insert(tag);

        let key = [0xB5_u8; 32];
        let payload = b"fallback lane delivery";
        let encoded_object = make_encoded_object(payload, tag, &key);
        let root = blake3_32(&encoded_object);
        let shards = object_to_shards(&encoded_object, Namespace(8), Epoch(43), tag, root)
            .expect("sharding should succeed");
        let k = shards[0].header.k as usize;

        let mut fast = InMemoryAdapter::default();
        let mut fallback = InMemoryAdapter::default();
        for shard in shards.iter().take(k) {
            let bytes = encode_shard_cbor(shard).expect("shard should encode");
            fallback.enqueue_inbound("sender", bytes);
        }

        let peers = vec![
            "sender".to_string(),
            "peer-a".to_string(),
            "peer-b".to_string(),
        ];
        let mut stats = RuntimeStats::default();
        let mut delivered = false;
        for step in 0..k {
            let event = pump_multi_lane_once(
                &mut node,
                &mut fast,
                &mut fallback,
                MultiLanePumpParams {
                    fast_lane: LaneForwardParams {
                        peers: &peers,
                        fanout: 2,
                    },
                    fallback_lane: LaneForwardParams {
                        peers: &peers,
                        fanout: 1,
                    },
                    fallback_redundancy_fanout: 1,
                    now_step: step as u64,
                    ttl_steps: 100,
                    policy_hooks: RuntimePolicyHooks::default(),
                    decrypt_key: &key,
                    stats: &mut stats,
                },
                &XChaCha20Poly1305Cipher,
                &Ed25519Verifier,
            )
            .expect("multi-lane pump should succeed");
            if let Some(crate::receive::ReceiveEvent::Delivered { payload: got, .. }) = event {
                assert_eq!(got, payload.to_vec());
                delivered = true;
            }
        }
        assert!(delivered, "expected delivery via fallback lane ingest");
    }

    #[test]
    fn multi_lane_pump_can_redundantly_forward_fast_lane_bytes_to_fallback() {
        let mut node = NodeState::default();
        let tag = [0x31_u8; 32];
        node.subscriptions.insert(tag);

        let key = [0xC5_u8; 32];
        let payload = b"redundant forward";
        let encoded_object = make_encoded_object(payload, tag, &key);
        let root = blake3_32(&encoded_object);
        let shards = object_to_shards(&encoded_object, Namespace(9), Epoch(44), tag, root)
            .expect("sharding should succeed");

        let mut fast = InMemoryAdapter::default();
        let mut fallback = InMemoryAdapter::default();
        let bytes = encode_shard_cbor(&shards[0]).expect("shard should encode");
        fast.enqueue_inbound("sender", bytes);

        let peers = vec![
            "sender".to_string(),
            "peer-a".to_string(),
            "peer-b".to_string(),
        ];
        let mut stats = RuntimeStats::default();

        let _ = pump_multi_lane_once(
            &mut node,
            &mut fast,
            &mut fallback,
            MultiLanePumpParams {
                fast_lane: LaneForwardParams {
                    peers: &peers,
                    fanout: 2,
                },
                fallback_lane: LaneForwardParams {
                    peers: &peers,
                    fanout: 1,
                },
                fallback_redundancy_fanout: 1,
                now_step: 0,
                ttl_steps: 100,
                policy_hooks: RuntimePolicyHooks::default(),
                decrypt_key: &key,
                stats: &mut stats,
            },
            &XChaCha20Poly1305Cipher,
            &Ed25519Verifier,
        )
        .expect("multi-lane pump should succeed");

        let fallback_outbound = fallback.take_outbound();
        assert!(
            !fallback_outbound.is_empty(),
            "expected fallback lane to receive redundant forwards",
        );
    }

    #[test]
    fn policy_hooks_can_reduce_fanout_for_blocked_peers() {
        let mut node = NodeState::default();
        let tag = [0x41_u8; 32];
        node.subscriptions.insert(tag);
        let key = [0xD5_u8; 32];

        let payload = b"policy fanout";
        let encoded_object = make_encoded_object(payload, tag, &key);
        let root = blake3_32(&encoded_object);
        let shards = object_to_shards(&encoded_object, Namespace(10), Epoch(45), tag, root)
            .expect("sharding should succeed");
        let bytes = encode_shard_cbor(&shards[0]).expect("shard should encode");

        let mut adapter = InMemoryAdapter::default();
        adapter.enqueue_inbound("blocked-peer", bytes);

        let peers = vec![
            "blocked-peer".to_string(),
            "peer-a".to_string(),
            "peer-b".to_string(),
        ];
        let mut stats = RuntimeStats::default();
        let mut cfg = NodeRuntimeConfig::default();
        let blocked_pubkey = [0x99_u8; 32];
        cfg.wot_policy.block(blocked_pubkey);
        cfg.bind_peer_publisher("blocked-peer", blocked_pubkey);
        let fanout_fn =
            |peer: &String, now_step: u64, base: usize| cfg.fanout_for_peer(peer, now_step, base);

        let _ = pump_once(
            &mut node,
            &mut adapter,
            PumpParams {
                peers: &peers,
                now_step: 0,
                ttl_steps: 100,
                fanout: 3,
                policy_hooks: RuntimePolicyHooks {
                    fanout_for_peer: Some(&fanout_fn),
                    ..RuntimePolicyHooks::default()
                },
                decrypt_key: &key,
                stats: &mut stats,
            },
            &XChaCha20Poly1305Cipher,
            &Ed25519Verifier,
        )
        .expect("pump should succeed");

        let outbound = adapter.take_outbound();
        assert!(
            outbound.is_empty(),
            "blocked sender should get zero forwarding fanout",
        );
    }

    #[test]
    fn config_wrapper_applies_policy_without_manual_hook_wiring() {
        let mut node = NodeState::default();
        let tag = [0x51_u8; 32];
        node.subscriptions.insert(tag);
        let key = [0xE5_u8; 32];
        let payload = b"config wrapper fanout";
        let encoded_object = make_encoded_object(payload, tag, &key);
        let root = blake3_32(&encoded_object);
        let shards = object_to_shards(&encoded_object, Namespace(11), Epoch(46), tag, root)
            .expect("sharding should succeed");
        let bytes = encode_shard_cbor(&shards[0]).expect("shard should encode");

        let mut adapter = InMemoryAdapter::default();
        adapter.enqueue_inbound("blocked-peer", bytes);
        let peers = vec![
            "blocked-peer".to_string(),
            "peer-a".to_string(),
            "peer-b".to_string(),
        ];
        let mut stats = RuntimeStats::default();

        let mut cfg = NodeRuntimeConfig::default();
        let blocked_pubkey = [0x98_u8; 32];
        cfg.wot_policy.block(blocked_pubkey);
        cfg.bind_peer_publisher("blocked-peer", blocked_pubkey);
        cfg.base_fast_fanout = 3;

        let _ = pump_once_with_config(
            &mut node,
            &mut adapter,
            ConfigPumpParams {
                peers: &peers,
                now_step: 0,
                decrypt_key: &key,
                config: &cfg,
                stats: &mut stats,
            },
            &XChaCha20Poly1305Cipher,
            &Ed25519Verifier,
        )
        .expect("wrapper pump should succeed");

        assert!(adapter.take_outbound().is_empty());
    }

    #[test]
    fn multi_lane_config_wrapper_runs() {
        let mut node = NodeState::default();
        let tag = [0x61_u8; 32];
        node.subscriptions.insert(tag);
        let key = [0xF5_u8; 32];
        let payload = b"config multi lane";
        let encoded_object = make_encoded_object(payload, tag, &key);
        let root = blake3_32(&encoded_object);
        let shards = object_to_shards(&encoded_object, Namespace(12), Epoch(47), tag, root)
            .expect("sharding should succeed");
        let k = shards[0].header.k as usize;

        let mut fast = InMemoryAdapter::default();
        let mut fallback = InMemoryAdapter::default();
        for shard in shards.iter().take(k) {
            fallback.enqueue_inbound(
                "sender",
                encode_shard_cbor(shard).expect("shard should encode"),
            );
        }

        let peers = vec![
            "sender".to_string(),
            "peer-a".to_string(),
            "peer-b".to_string(),
        ];
        let mut stats = RuntimeStats::default();
        let cfg = NodeRuntimeConfig::default();
        let mut delivered = false;

        for step in 0..k {
            let event = pump_multi_lane_once_with_config(
                &mut node,
                &mut fast,
                &mut fallback,
                ConfigMultiLanePumpParams {
                    fast_peers: &peers,
                    fallback_peers: &peers,
                    now_step: step as u64,
                    decrypt_key: &key,
                    config: &cfg,
                    stats: &mut stats,
                },
                &XChaCha20Poly1305Cipher,
                &Ed25519Verifier,
            )
            .expect("multi-lane wrapper should succeed");

            if matches!(event, Some(crate::receive::ReceiveEvent::Delivered { .. })) {
                delivered = true;
            }
        }
        assert!(delivered);
    }

    #[test]
    fn ack_timeout_pump_sends_escalation_batches_on_fallback_lane() {
        let mut node = NodeState::default();
        let root = [0xD5; 32];
        register_pending_ack(
            &mut node,
            root,
            vec![vec![1, 2, 3], vec![4, 5, 6], vec![7, 8, 9]],
            10,
            AckRetryPolicy {
                initial_timeout_steps: 2,
                retry_batch_size: 2,
                backoff_step: 2,
                max_retries: 3,
            },
        );

        let peers = vec!["peer-a".to_string(), "peer-b".to_string()];
        let mut fallback = InMemoryAdapter::default();
        let mut stats = RuntimeStats::default();

        let sent_before = pump_ack_timeouts(&mut node, &mut fallback, &peers, 11, 1, &mut stats);
        assert_eq!(sent_before, 0);

        let sent_after = pump_ack_timeouts(&mut node, &mut fallback, &peers, 12, 1, &mut stats);
        assert_eq!(sent_after, 2);
        assert!(node.pending_acks.contains_key(&root));

        let sent_final = pump_ack_timeouts(&mut node, &mut fallback, &peers, 14, 1, &mut stats);
        assert_eq!(sent_final, 1);
        assert!(!node.pending_acks.contains_key(&root));
    }

    #[test]
    fn runtime_clears_pending_ack_when_ack_payload_is_delivered() {
        let mut node = NodeState::default();
        let tag = [0x71_u8; 32];
        node.subscriptions.insert(tag);

        let target_root = [0xA7_u8; 32];
        register_pending_ack(
            &mut node,
            target_root,
            vec![vec![1, 2, 3]],
            0,
            AckRetryPolicy {
                initial_timeout_steps: 2,
                retry_batch_size: 1,
                backoff_step: 1,
                max_retries: 1,
            },
        );
        assert!(node.pending_acks.contains_key(&target_root));

        let key = [0xA9_u8; 32];
        let ack_payload = encode_ack_payload(target_root);
        let encoded_object = make_encoded_object(&ack_payload, tag, &key);
        let root = blake3_32(&encoded_object);
        let shards = object_to_shards(&encoded_object, Namespace(13), Epoch(48), tag, root)
            .expect("sharding should succeed");
        let k = shards[0].header.k as usize;

        let mut adapter = InMemoryAdapter::default();
        for shard in shards.iter().take(k) {
            adapter.enqueue_inbound(
                "sender",
                encode_shard_cbor(shard).expect("shard should encode"),
            );
        }

        let peers = vec![
            "sender".to_string(),
            "peer-a".to_string(),
            "peer-b".to_string(),
        ];
        let mut stats = RuntimeStats::default();
        for step in 0..k {
            let _ = pump_once(
                &mut node,
                &mut adapter,
                PumpParams {
                    peers: &peers,
                    now_step: step as u64,
                    ttl_steps: 50,
                    fanout: 2,
                    policy_hooks: RuntimePolicyHooks::default(),
                    decrypt_key: &key,
                    stats: &mut stats,
                },
                &XChaCha20Poly1305Cipher,
                &Ed25519Verifier,
            )
            .expect("pump should succeed");
        }

        assert!(!node.pending_acks.contains_key(&target_root));
        assert_eq!(stats.ack_messages, 1);
    }

    #[test]
    fn runtime_auto_emits_ack_shards_when_ack_is_requested() {
        let mut node = NodeState::default();
        let tag = [0x81_u8; 32];
        node.subscriptions.insert(tag);

        let key = [0xB9_u8; 32];
        let payload = b"please ack me".to_vec();
        let encoded_object = make_encoded_object_with_flags(
            &payload,
            tag,
            &key,
            OBJECT_FLAG_SIGNED | OBJECT_FLAG_ACK_REQUESTED,
        );
        let root = blake3_32(&encoded_object);
        let shards = object_to_shards(&encoded_object, Namespace(14), Epoch(49), tag, root)
            .expect("sharding should succeed");
        let k = shards[0].header.k as usize;

        let mut adapter = InMemoryAdapter::default();
        for shard in shards.iter().take(k) {
            adapter.enqueue_inbound(
                "sender",
                encode_shard_cbor(shard).expect("shard should encode"),
            );
        }

        let peers = vec![
            "sender".to_string(),
            "peer-a".to_string(),
            "peer-b".to_string(),
        ];
        let mut stats = RuntimeStats::default();
        for step in 0..k {
            let _ = pump_once(
                &mut node,
                &mut adapter,
                PumpParams {
                    peers: &peers,
                    now_step: step as u64,
                    ttl_steps: 50,
                    fanout: 2,
                    policy_hooks: RuntimePolicyHooks::default(),
                    decrypt_key: &key,
                    stats: &mut stats,
                },
                &XChaCha20Poly1305Cipher,
                &Ed25519Verifier,
            )
            .expect("pump should succeed");
        }

        let outbound = adapter.take_outbound();
        assert!(
            !outbound.is_empty(),
            "expected runtime to emit ack shard(s) to sender",
        );
    }

    #[test]
    fn tick_wrapper_runs_due_ack_retries_automatically() {
        let mut node = NodeState::default();
        let root = [0xE1; 32];
        register_pending_ack(
            &mut node,
            root,
            vec![vec![1, 2, 3], vec![4, 5, 6]],
            0,
            AckRetryPolicy {
                initial_timeout_steps: 1,
                retry_batch_size: 1,
                backoff_step: 1,
                max_retries: 3,
            },
        );

        let mut fast = InMemoryAdapter::default();
        let mut fallback = InMemoryAdapter::default();
        let peers = vec![
            "sender".to_string(),
            "peer-a".to_string(),
            "peer-b".to_string(),
        ];
        let mut stats = RuntimeStats::default();
        let cfg = NodeRuntimeConfig::default();

        let event = pump_multi_lane_tick_with_config(
            &mut node,
            &mut fast,
            &mut fallback,
            ConfigMultiLanePumpParams {
                fast_peers: &peers,
                fallback_peers: &peers,
                now_step: 1,
                decrypt_key: &[0xAA; 32],
                config: &cfg,
                stats: &mut stats,
            },
            &XChaCha20Poly1305Cipher,
            &Ed25519Verifier,
        )
        .expect("tick wrapper should succeed");

        assert!(event.is_none());
        assert!(!fallback.take_outbound().is_empty());
    }
}
