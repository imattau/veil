use std::collections::HashSet;
use veil_codec::object::OBJECT_FLAG_ACK_REQUESTED;
use veil_codec::shard::decode_shard_cbor;
use veil_core::hash::blake3_32;
use veil_crypto::aead::AeadCipher;
use veil_crypto::signing::Verifier;
use veil_fec::profile::ErasureCodingMode;
use veil_transport::adapter::TransportAdapter;

use crate::ack::{
    ack_received, build_ack_shard_bytes_with_mode_and_padding, decode_ack_payload,
    next_ack_escalation_batch,
};
use crate::bloom::decode_bloom_exchange_packet;
use crate::config::{NodeRuntimeConfig, ProbabilisticForwardingConfig};
use crate::policy::{TrustTier, WotPolicy};
use crate::receive::{receive_shard_with_policy, ReceiveCachePolicy, ReceiveError, ReceiveEvent};
use crate::state::NodeState;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct TierCounters {
    pub trusted: usize,
    pub known: usize,
    pub unknown: usize,
    pub muted: usize,
    pub blocked: usize,
}

impl TierCounters {
    fn incr(&mut self, tier: TrustTier, value: usize) {
        match tier {
            TrustTier::Trusted => self.trusted = self.trusted.saturating_add(value),
            TrustTier::Known => self.known = self.known.saturating_add(value),
            TrustTier::Unknown => self.unknown = self.unknown.saturating_add(value),
            TrustTier::Muted => self.muted = self.muted.saturating_add(value),
            TrustTier::Blocked => self.blocked = self.blocked.saturating_add(value),
        }
    }
}

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
    /// Inbound payloads that failed shard decode.
    pub malformed_messages: usize,
    /// Inbound control-plane Bloom exchange packets.
    pub bloom_messages: usize,
    /// Outbound send attempts that failed at transport level.
    pub send_failures: usize,
    /// Inbound message counts grouped by source trust tier.
    pub inbound_by_tier: TierCounters,
    /// Successful forwards grouped by source trust tier.
    pub forwarded_by_tier: TierCounters,
    /// Forward opportunities not used due fanout/quota limits.
    pub dropped_by_tier: TierCounters,
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
pub struct MultiLanePumpParams<'a, PFast, PFallback> {
    pub fast_lane: LaneForwardParams<'a, PFast>,
    pub fallback_lane: LaneForwardParams<'a, PFallback>,
    pub fallback_redundancy_fanout: usize,
    pub now_step: u64,
    pub ttl_steps: u64,
    pub fast_policy_hooks: RuntimePolicyHooks<'a, PFast>,
    pub fallback_policy_hooks: RuntimePolicyHooks<'a, PFallback>,
    pub decrypt_key: &'a [u8; 32],
    pub stats: &'a mut RuntimeStats,
}

/// Configuration-driven wrapper parameters for `pump_multi_lane_once_with_config`.
pub struct ConfigMultiLanePumpParams<'a, PFast, PFallback> {
    pub fast_peers: &'a [PFast],
    pub fallback_peers: &'a [PFallback],
    pub now_step: u64,
    pub decrypt_key: &'a [u8; 32],
    pub config: &'a NodeRuntimeConfig,
    pub stats: &'a mut RuntimeStats,
}

/// Optional callback used to adjust fanout per inbound peer.
pub type PeerFanoutFn<'a, P> = dyn Fn(&P, u64, usize) -> usize + 'a;
/// Optional callback used to classify peer trust tier for cache policy.
pub type PeerTierFn<'a, P> = dyn Fn(&P, u64) -> TrustTier + 'a;
/// Optional callback resolving an inbound peer to publisher pubkey.
pub type PeerPublisherResolver<'a, P> = dyn Fn(&P) -> Option<[u8; 32]> + 'a;

#[derive(Clone, Copy)]
pub struct RuntimePolicyHooks<'a, P> {
    pub fanout_for_peer: Option<&'a PeerFanoutFn<'a, P>>,
    pub classify_peer_tier: Option<&'a PeerTierFn<'a, P>>,
    pub max_cache_shards: usize,
    pub wot_policy: Option<&'a dyn WotPolicy>,
    pub erasure_coding_mode: ErasureCodingMode,
    pub bucket_jitter_extra_levels: usize,
    pub required_signed_namespaces: Option<&'a HashSet<u16>>,
    pub probabilistic_forwarding: ProbabilisticForwardingConfig,
    pub accept_all_tags: bool,
}

impl<'a, P> Default for RuntimePolicyHooks<'a, P> {
    fn default() -> Self {
        Self {
            fanout_for_peer: None,
            classify_peer_tier: None,
            max_cache_shards: usize::MAX,
            wot_policy: None,
            erasure_coding_mode: ErasureCodingMode::HardenedNonSystematic,
            bucket_jitter_extra_levels: 0,
            required_signed_namespaces: None,
            probabilistic_forwarding: ProbabilisticForwardingConfig::default(),
            accept_all_tags: false,
        }
    }
}

struct InboundProcessParams<'a, P> {
    from_peer: &'a P,
    bytes: &'a [u8],
    peers: &'a [P],
    fanout: usize,
    now_step: u64,
    inbound_tier: TrustTier,
    classify_peer_tier: Option<&'a PeerTierFn<'a, P>>,
    ttl_steps: u64,
    decrypt_key: &'a [u8; 32],
    cache_policy: Option<ReceiveCachePolicy<'a>>,
    probabilistic_forwarding: ProbabilisticForwardingConfig,
    stats: &'a mut RuntimeStats,
}

fn is_forwardable(event: &ReceiveEvent) -> bool {
    matches!(
        event,
        ReceiveEvent::Buffered { .. } | ReceiveEvent::Delivered { .. }
    )
}

fn forwarding_probability(replica_estimate: u64, cfg: ProbabilisticForwardingConfig) -> f64 {
    if !cfg.enabled {
        return 1.0;
    }
    let divisor = cfg.replica_divisor.max(1) as f64;
    let p = 1.0 / (1.0 + (replica_estimate as f64 / divisor));
    p.max(cfg.min_probability.clamp(0.0, 1.0))
}

fn probabilistic_allow(
    shard_id: [u8; 32],
    peer_ordinal: usize,
    now_step: u64,
    probability: f64,
) -> bool {
    if probability >= 1.0 {
        return true;
    }
    if probability <= 0.0 {
        return false;
    }
    let mut preimage = Vec::with_capacity(32 + 8 + 8 + 10);
    preimage.extend_from_slice(b"pfwd-v1");
    preimage.extend_from_slice(&shard_id);
    preimage.extend_from_slice(&(peer_ordinal as u64).to_be_bytes());
    preimage.extend_from_slice(&now_step.to_be_bytes());
    let sample = blake3_32(&preimage);
    let draw = u16::from_be_bytes([sample[0], sample[1]]) as f64 / u16::MAX as f64;
    draw <= probability
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
        inbound_tier,
        classify_peer_tier,
        ttl_steps,
        decrypt_key,
        cache_policy,
        probabilistic_forwarding,
        stats,
    } = params;
    let sid = blake3_32(bytes);

    stats.inbound_messages += 1;
    stats.inbound_by_tier.incr(inbound_tier, 1);

    if decode_bloom_exchange_packet(bytes).is_some() {
        stats.bloom_messages += 1;
        stats.ignored_messages += 1;
        return Ok(ReceiveEvent::IgnoredMalformed);
    }

    let shard = match decode_shard_cbor(bytes) {
        Ok(shard) => shard,
        Err(_) => {
            stats.ignored_messages += 1;
            stats.malformed_messages += 1;
            return Ok(ReceiveEvent::IgnoredMalformed);
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
        let mut candidates = peers
            .iter()
            .filter(|peer| **peer != *from_peer)
            .collect::<Vec<_>>();
        if let Some(classify) = classify_peer_tier {
            candidates.sort_by_key(|peer| match classify(peer, now_step) {
                TrustTier::Trusted => 0_u8,
                TrustTier::Known => 1_u8,
                TrustTier::Unknown => 2_u8,
                TrustTier::Muted => 3_u8,
                TrustTier::Blocked => 4_u8,
            });
        }
        stats
            .dropped_by_tier
            .incr(inbound_tier, candidates.len().saturating_sub(fanout));
        for (ordinal, peer) in candidates.into_iter().take(fanout).enumerate() {
            let replica = *node.replica_estimate.get(&sid).unwrap_or(&0);
            let p = forwarding_probability(replica, probabilistic_forwarding);
            if !probabilistic_allow(sid, ordinal, now_step, p) {
                continue;
            }
            if adapter.send(peer, bytes).is_ok() {
                stats.forwarded_messages += 1;
                stats.forwarded_by_tier.incr(inbound_tier, 1);
            } else {
                stats.send_failures += 1;
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
            let ack_mode = cache_policy
                .as_ref()
                .map(|p| p.erasure_coding_mode)
                .unwrap_or(ErasureCodingMode::HardenedNonSystematic);
            let ack_bucket_jitter = cache_policy
                .as_ref()
                .map(|p| p.bucket_jitter_extra_levels)
                .unwrap_or(0);
            if let Ok(ack_shards) = build_ack_shard_bytes_with_mode_and_padding(
                *object_root,
                *tag,
                *namespace,
                *epoch,
                decrypt_key,
                cipher,
                ack_mode,
                ack_bucket_jitter,
            ) {
                for ack_shard in &ack_shards {
                    if adapter.send(from_peer, ack_shard).is_ok() {
                        stats.forwarded_messages += 1;
                    } else {
                        stats.send_failures += 1;
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
    let inbound_tier = policy_hooks
        .classify_peer_tier
        .map(|f| f(&from_peer, now_step))
        .unwrap_or(TrustTier::Unknown);
    let cache_policy = policy_hooks
        .wot_policy
        .map(|wot_policy| ReceiveCachePolicy {
            tier: inbound_tier,
            max_cache_shards: policy_hooks.max_cache_shards,
            wot_policy,
            erasure_coding_mode: policy_hooks.erasure_coding_mode,
            bucket_jitter_extra_levels: policy_hooks.bucket_jitter_extra_levels,
            required_signed_namespaces: policy_hooks.required_signed_namespaces,
            probabilistic_forwarding: policy_hooks.probabilistic_forwarding,
            accept_all_tags: policy_hooks.accept_all_tags,
        });
    let event = process_inbound(
        node,
        adapter,
        InboundProcessParams {
            from_peer: &from_peer,
            bytes: &bytes,
            peers,
            fanout: effective_fanout,
            now_step,
            inbound_tier,
            classify_peer_tier: policy_hooks.classify_peer_tier,
            ttl_steps,
            decrypt_key,
            cache_policy,
            probabilistic_forwarding: policy_hooks.probabilistic_forwarding,
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
    let resolver = |peer: &A::Peer| config.publisher_for_peer(&peer.to_string());
    pump_once_with_config_resolver(
        node,
        adapter,
        ConfigPumpParams {
            peers,
            now_step,
            decrypt_key,
            config,
            stats,
        },
        &resolver,
        cipher,
        verifier,
    )
}

/// Convenience wrapper around `pump_once` using `NodeRuntimeConfig` and an
/// external peer->publisher resolver.
pub fn pump_once_with_config_resolver<A: TransportAdapter>(
    node: &mut NodeState,
    adapter: &mut A,
    params: ConfigPumpParams<'_, A::Peer>,
    resolver: &PeerPublisherResolver<'_, A::Peer>,
    cipher: &impl AeadCipher,
    verifier: &impl Verifier,
) -> Result<Option<ReceiveEvent>, ReceiveError> {
    let ConfigPumpParams {
        peers,
        now_step,
        decrypt_key,
        config,
        stats,
    } = params;

    let fanout_fn = |peer: &A::Peer, step: u64, base: usize| {
        let tier = config.classify_publisher_tier(resolver(peer), step);
        config.fanout_for_tier(tier, base)
    };
    let tier_fn = |peer: &A::Peer, step: u64| config.classify_publisher_tier(resolver(peer), step);

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
                erasure_coding_mode: config.erasure_coding_mode,
                bucket_jitter_extra_levels: config.bucket_jitter_extra_levels,
                required_signed_namespaces: Some(&config.required_signed_namespaces),
                probabilistic_forwarding: config.probabilistic_forwarding,
                accept_all_tags: config.accept_all_tags,
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
    params: MultiLanePumpParams<'_, A::Peer, A::Peer>,
    cipher: &impl AeadCipher,
    verifier: &impl Verifier,
) -> Result<Option<ReceiveEvent>, ReceiveError> {
    pump_multi_lane_once_split(
        node,
        fast_adapter,
        fallback_adapter,
        params,
        cipher,
        verifier,
    )
}

/// Runs one multi-lane runtime step with independent adapter types per lane.
pub fn pump_multi_lane_once_split<AFast: TransportAdapter, AFallback: TransportAdapter>(
    node: &mut NodeState,
    fast_adapter: &mut AFast,
    fallback_adapter: &mut AFallback,
    params: MultiLanePumpParams<'_, AFast::Peer, AFallback::Peer>,
    cipher: &impl AeadCipher,
    verifier: &impl Verifier,
) -> Result<Option<ReceiveEvent>, ReceiveError> {
    let MultiLanePumpParams {
        fast_lane,
        fallback_lane,
        fallback_redundancy_fanout,
        now_step,
        ttl_steps,
        fast_policy_hooks,
        fallback_policy_hooks,
        decrypt_key,
        stats,
    } = params;

    if let Some((from_peer, bytes)) = fast_adapter.recv() {
        let effective_fast_fanout = fast_policy_hooks
            .fanout_for_peer
            .map(|f| f(&from_peer, now_step, fast_lane.fanout))
            .unwrap_or(fast_lane.fanout);
        let cache_policy = match (
            fast_policy_hooks.classify_peer_tier,
            fast_policy_hooks.wot_policy,
        ) {
            (Some(classify_peer_tier), Some(wot_policy)) => Some(ReceiveCachePolicy {
                tier: classify_peer_tier(&from_peer, now_step),
                max_cache_shards: fast_policy_hooks.max_cache_shards,
                wot_policy,
                erasure_coding_mode: fast_policy_hooks.erasure_coding_mode,
                bucket_jitter_extra_levels: fast_policy_hooks.bucket_jitter_extra_levels,
                required_signed_namespaces: fast_policy_hooks.required_signed_namespaces,
                probabilistic_forwarding: fast_policy_hooks.probabilistic_forwarding,
                accept_all_tags: fast_policy_hooks.accept_all_tags,
            }),
            _ => None,
        };
        let inbound_tier = fast_policy_hooks
            .classify_peer_tier
            .map(|f| f(&from_peer, now_step))
            .unwrap_or(TrustTier::Unknown);
        let event = process_inbound(
            node,
            fast_adapter,
            InboundProcessParams {
                from_peer: &from_peer,
                bytes: &bytes,
                peers: fast_lane.peers,
                fanout: effective_fast_fanout,
                now_step,
                inbound_tier,
                classify_peer_tier: fast_policy_hooks.classify_peer_tier,
                ttl_steps,
                decrypt_key,
                cache_policy,
                probabilistic_forwarding: fast_policy_hooks.probabilistic_forwarding,
                stats,
            },
            cipher,
            verifier,
        )?;

        let effective_redundancy = fast_policy_hooks
            .fanout_for_peer
            .map(|f| f(&from_peer, now_step, fallback_redundancy_fanout))
            .unwrap_or(fallback_redundancy_fanout);

        if is_forwardable(&event) && effective_redundancy > 0 {
            stats.dropped_by_tier.incr(
                inbound_tier,
                fallback_lane
                    .peers
                    .len()
                    .saturating_sub(effective_redundancy),
            );
            for peer in fallback_lane.peers.iter().take(effective_redundancy) {
                if fallback_adapter.send(peer, &bytes).is_ok() {
                    stats.forwarded_messages += 1;
                    stats.forwarded_by_tier.incr(inbound_tier, 1);
                } else {
                    stats.send_failures += 1;
                }
            }
        }
        return Ok(Some(event));
    }

    if let Some((from_peer, bytes)) = fallback_adapter.recv() {
        let effective_fallback_fanout = fallback_policy_hooks
            .fanout_for_peer
            .map(|f| f(&from_peer, now_step, fallback_lane.fanout))
            .unwrap_or(fallback_lane.fanout);
        let cache_policy = match (
            fallback_policy_hooks.classify_peer_tier,
            fallback_policy_hooks.wot_policy,
        ) {
            (Some(classify_peer_tier), Some(wot_policy)) => Some(ReceiveCachePolicy {
                tier: classify_peer_tier(&from_peer, now_step),
                max_cache_shards: fallback_policy_hooks.max_cache_shards,
                wot_policy,
                erasure_coding_mode: fallback_policy_hooks.erasure_coding_mode,
                bucket_jitter_extra_levels: fallback_policy_hooks.bucket_jitter_extra_levels,
                required_signed_namespaces: fallback_policy_hooks.required_signed_namespaces,
                probabilistic_forwarding: fallback_policy_hooks.probabilistic_forwarding,
                accept_all_tags: fallback_policy_hooks.accept_all_tags,
            }),
            _ => None,
        };
        let inbound_tier = fallback_policy_hooks
            .classify_peer_tier
            .map(|f| f(&from_peer, now_step))
            .unwrap_or(TrustTier::Unknown);
        let event = process_inbound(
            node,
            fallback_adapter,
            InboundProcessParams {
                from_peer: &from_peer,
                bytes: &bytes,
                peers: fallback_lane.peers,
                fanout: effective_fallback_fanout,
                now_step,
                inbound_tier,
                classify_peer_tier: fallback_policy_hooks.classify_peer_tier,
                ttl_steps,
                decrypt_key,
                cache_policy,
                probabilistic_forwarding: fallback_policy_hooks.probabilistic_forwarding,
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
    params: ConfigMultiLanePumpParams<'_, A::Peer, A::Peer>,
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
    let resolver = |peer: &A::Peer| config.publisher_for_peer(&peer.to_string());
    pump_multi_lane_once_with_config_resolvers_split(
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
        &resolver,
        &resolver,
        cipher,
        verifier,
    )
}

/// Convenience wrapper around `pump_multi_lane_once_split` using
/// `NodeRuntimeConfig` and explicit peer->publisher resolvers.
#[allow(clippy::too_many_arguments)]
pub fn pump_multi_lane_once_with_config_resolvers_split<AFast, AFallback>(
    node: &mut NodeState,
    fast_adapter: &mut AFast,
    fallback_adapter: &mut AFallback,
    params: ConfigMultiLanePumpParams<'_, AFast::Peer, AFallback::Peer>,
    fast_resolver: &PeerPublisherResolver<'_, AFast::Peer>,
    fallback_resolver: &PeerPublisherResolver<'_, AFallback::Peer>,
    cipher: &impl AeadCipher,
    verifier: &impl Verifier,
) -> Result<Option<ReceiveEvent>, ReceiveError>
where
    AFast: TransportAdapter,
    AFallback: TransportAdapter,
{
    let ConfigMultiLanePumpParams {
        fast_peers,
        fallback_peers,
        now_step,
        decrypt_key,
        config,
        stats,
    } = params;

    let fast_fanout_fn = |peer: &AFast::Peer, step: u64, base: usize| {
        let tier = config.classify_publisher_tier(fast_resolver(peer), step);
        config.fanout_for_tier(tier, base)
    };
    let fast_tier_fn =
        |peer: &AFast::Peer, step: u64| config.classify_publisher_tier(fast_resolver(peer), step);
    let fallback_fanout_fn = |peer: &AFallback::Peer, step: u64, base: usize| {
        let tier = config.classify_publisher_tier(fallback_resolver(peer), step);
        config.fanout_for_tier(tier, base)
    };
    let fallback_tier_fn = |peer: &AFallback::Peer, step: u64| {
        config.classify_publisher_tier(fallback_resolver(peer), step)
    };

    pump_multi_lane_once_split(
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
            fast_policy_hooks: RuntimePolicyHooks {
                fanout_for_peer: Some(&fast_fanout_fn),
                classify_peer_tier: Some(&fast_tier_fn),
                max_cache_shards: config.max_cache_shards,
                wot_policy: Some(&config.wot_policy),
                erasure_coding_mode: config.erasure_coding_mode,
                bucket_jitter_extra_levels: config.bucket_jitter_extra_levels,
                required_signed_namespaces: Some(&config.required_signed_namespaces),
                probabilistic_forwarding: config.probabilistic_forwarding,
                accept_all_tags: config.accept_all_tags,
            },
            fallback_policy_hooks: RuntimePolicyHooks {
                fanout_for_peer: Some(&fallback_fanout_fn),
                classify_peer_tier: Some(&fallback_tier_fn),
                max_cache_shards: config.max_cache_shards,
                wot_policy: Some(&config.wot_policy),
                erasure_coding_mode: config.erasure_coding_mode,
                bucket_jitter_extra_levels: config.bucket_jitter_extra_levels,
                required_signed_namespaces: Some(&config.required_signed_namespaces),
                probabilistic_forwarding: config.probabilistic_forwarding,
                accept_all_tags: config.accept_all_tags,
            },
            decrypt_key,
            stats,
        },
        cipher,
        verifier,
    )
}

pub fn pump_multi_lane_once_with_config_split<AFast, AFallback>(
    node: &mut NodeState,
    fast_adapter: &mut AFast,
    fallback_adapter: &mut AFallback,
    params: ConfigMultiLanePumpParams<'_, AFast::Peer, AFallback::Peer>,
    cipher: &impl AeadCipher,
    verifier: &impl Verifier,
) -> Result<Option<ReceiveEvent>, ReceiveError>
where
    AFast: TransportAdapter,
    AFallback: TransportAdapter,
    AFast::Peer: ToString,
    AFallback::Peer: ToString,
{
    let ConfigMultiLanePumpParams {
        fast_peers,
        fallback_peers,
        now_step,
        decrypt_key,
        config,
        stats,
    } = params;
    let fast_resolver = |peer: &AFast::Peer| config.publisher_for_peer(&peer.to_string());
    let fallback_resolver = |peer: &AFallback::Peer| config.publisher_for_peer(&peer.to_string());
    pump_multi_lane_once_with_config_resolvers_split(
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
        &fast_resolver,
        &fallback_resolver,
        cipher,
        verifier,
    )
}

/// Runs one multi-lane runtime tick and then processes due ACK-timeout retries.
///
/// This is the recommended node-side runtime entrypoint:
/// - ingests one message (fast lane first, then fallback)
/// - applies forwarding/reconstruction/decrypt pipeline
/// - auto-emits ACK objects for delivered inbound objects with
///   `ack_requested`
/// - sends due retry shards for locally pending outbound ACK waits
pub fn pump_multi_lane_tick_with_config<A>(
    node: &mut NodeState,
    fast_adapter: &mut A,
    fallback_adapter: &mut A,
    params: ConfigMultiLanePumpParams<'_, A::Peer, A::Peer>,
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
    let resolver = |peer: &A::Peer| config.publisher_for_peer(&peer.to_string());
    pump_multi_lane_tick_with_config_resolvers_split(
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
        &resolver,
        &resolver,
        cipher,
        verifier,
    )
}

/// Runs one multi-lane runtime tick for independent adapter types and then
/// processes due ACK-timeout retries using explicit peer resolvers.
#[allow(clippy::too_many_arguments)]
pub fn pump_multi_lane_tick_with_config_resolvers_split<AFast, AFallback>(
    node: &mut NodeState,
    fast_adapter: &mut AFast,
    fallback_adapter: &mut AFallback,
    params: ConfigMultiLanePumpParams<'_, AFast::Peer, AFallback::Peer>,
    fast_resolver: &PeerPublisherResolver<'_, AFast::Peer>,
    fallback_resolver: &PeerPublisherResolver<'_, AFallback::Peer>,
    cipher: &impl AeadCipher,
    verifier: &impl Verifier,
) -> Result<Option<ReceiveEvent>, ReceiveError>
where
    AFast: TransportAdapter,
    AFallback: TransportAdapter,
{
    let ConfigMultiLanePumpParams {
        fast_peers,
        fallback_peers,
        now_step,
        decrypt_key,
        config,
        stats,
    } = params;

    let event = pump_multi_lane_once_with_config_resolvers_split(
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
        fast_resolver,
        fallback_resolver,
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

/// Runs one multi-lane runtime tick for independent adapter types and then
/// processes due ACK-timeout retries.
pub fn pump_multi_lane_tick_with_config_split<AFast, AFallback>(
    node: &mut NodeState,
    fast_adapter: &mut AFast,
    fallback_adapter: &mut AFallback,
    params: ConfigMultiLanePumpParams<'_, AFast::Peer, AFallback::Peer>,
    cipher: &impl AeadCipher,
    verifier: &impl Verifier,
) -> Result<Option<ReceiveEvent>, ReceiveError>
where
    AFast: TransportAdapter,
    AFallback: TransportAdapter,
    AFast::Peer: ToString,
    AFallback::Peer: ToString,
{
    let ConfigMultiLanePumpParams {
        fast_peers,
        fallback_peers,
        now_step,
        decrypt_key,
        config,
        stats,
    } = params;
    let fast_resolver = |peer: &AFast::Peer| config.publisher_for_peer(&peer.to_string());
    let fallback_resolver = |peer: &AFallback::Peer| config.publisher_for_peer(&peer.to_string());
    pump_multi_lane_tick_with_config_resolvers_split(
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
        &fast_resolver,
        &fallback_resolver,
        cipher,
        verifier,
    )
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
                } else {
                    stats.send_failures += 1;
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
        forwarding_probability, probabilistic_allow, pump_ack_timeouts, pump_multi_lane_once,
        pump_multi_lane_once_with_config, pump_multi_lane_tick_with_config, pump_once,
        pump_once_with_config, ConfigMultiLanePumpParams, ConfigPumpParams, LaneForwardParams,
        MultiLanePumpParams, PumpParams, RuntimePolicyHooks, RuntimeStats,
    };
    use crate::ack::{encode_ack_payload, register_pending_ack, AckRetryPolicy};
    use crate::config::{NodeRuntimeConfig, ProbabilisticForwardingConfig};
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
            Some(crate::receive::ReceiveEvent::IgnoredMalformed)
        ));
        assert_eq!(stats.parsed_shards, 0);
        assert_eq!(stats.ignored_messages, 1);
        assert_eq!(stats.malformed_messages, 1);
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
                    fast_policy_hooks: RuntimePolicyHooks::default(),
                    fallback_policy_hooks: RuntimePolicyHooks::default(),
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
                fast_policy_hooks: RuntimePolicyHooks::default(),
                fallback_policy_hooks: RuntimePolicyHooks::default(),
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
        let tier_fn = |peer: &String, now_step: u64| cfg.classify_peer_tier(peer, now_step);

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
                    classify_peer_tier: Some(&tier_fn),
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
        assert_eq!(stats.inbound_by_tier.blocked, 1);
        assert_eq!(stats.forwarded_by_tier.blocked, 0);
        assert_eq!(stats.dropped_by_tier.blocked, 2);
    }

    #[test]
    fn probabilistic_forwarding_probability_drops_as_replica_estimate_rises() {
        let cfg = ProbabilisticForwardingConfig {
            enabled: true,
            min_probability: 0.10,
            replica_divisor: 8,
        };
        let rare = forwarding_probability(0, cfg);
        let common = forwarding_probability(80, cfg);
        assert!(rare > common);
        assert!(common >= 0.10);
    }

    #[test]
    fn probabilistic_allow_is_deterministic_for_same_inputs() {
        let sid = [0xAB; 32];
        let a = probabilistic_allow(sid, 3, 42, 0.35);
        let b = probabilistic_allow(sid, 3, 42, 0.35);
        assert_eq!(a, b);
    }

    #[test]
    fn probabilistic_forwarding_reduces_forward_traffic_for_common_shards() {
        let mut node_off = NodeState::default();
        let mut node_on = NodeState::default();
        let tag = [0x7A; 32];
        node_off.subscriptions.insert(tag);
        node_on.subscriptions.insert(tag);
        let key = [0xD7_u8; 32];

        let peers = vec![
            "origin".to_string(),
            "peer-a".to_string(),
            "peer-b".to_string(),
            "peer-c".to_string(),
            "peer-d".to_string(),
            "peer-e".to_string(),
        ];

        let mut adapter_off = InMemoryAdapter::default();
        let mut adapter_on = InMemoryAdapter::default();
        let mut stats_off = RuntimeStats::default();
        let mut stats_on = RuntimeStats::default();

        for i in 0..20u8 {
            let payload = vec![i; 128];
            let encoded = make_encoded_object(&payload, tag, &key);
            let root = blake3_32(&encoded);
            let shard = object_to_shards(&encoded, Namespace(20 + i as u16), Epoch(77), tag, root)
                .expect("sharding should succeed")
                .remove(0);
            let bytes = encode_shard_cbor(&shard).expect("shard encode");
            let sid = blake3_32(&bytes);

            // Mark as high-replica (common) to trigger aggressive downsampling.
            node_on.replica_estimate.insert(sid, 100);

            adapter_off.enqueue_inbound("origin", bytes.clone());
            adapter_on.enqueue_inbound("origin", bytes);
        }

        for step in 0..20u64 {
            let _ = pump_once(
                &mut node_off,
                &mut adapter_off,
                PumpParams {
                    peers: &peers,
                    now_step: step,
                    ttl_steps: 100,
                    fanout: 5,
                    policy_hooks: RuntimePolicyHooks::default(),
                    decrypt_key: &key,
                    stats: &mut stats_off,
                },
                &XChaCha20Poly1305Cipher,
                &Ed25519Verifier,
            )
            .expect("baseline pump should succeed");

            let _ = pump_once(
                &mut node_on,
                &mut adapter_on,
                PumpParams {
                    peers: &peers,
                    now_step: step,
                    ttl_steps: 100,
                    fanout: 5,
                    policy_hooks: RuntimePolicyHooks {
                        probabilistic_forwarding: ProbabilisticForwardingConfig {
                            enabled: true,
                            min_probability: 0.05,
                            replica_divisor: 1,
                        },
                        ..RuntimePolicyHooks::default()
                    },
                    decrypt_key: &key,
                    stats: &mut stats_on,
                },
                &XChaCha20Poly1305Cipher,
                &Ed25519Verifier,
            )
            .expect("probabilistic pump should succeed");
        }

        eprintln!(
            "probabilistic forwarding traffic estimate: baseline={}, probabilistic={}",
            stats_off.forwarded_messages, stats_on.forwarded_messages
        );
        assert!(stats_on.forwarded_messages < stats_off.forwarded_messages);
        // Estimated impact threshold for this synthetic "common shard" workload.
        assert!(stats_on.forwarded_messages * 2 < stats_off.forwarded_messages);
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
    fn config_wrapper_accept_all_tags_bypasses_subscription_gate() {
        let mut node = NodeState::default();
        let tag = [0x52_u8; 32];
        let key = [0xE6_u8; 32];
        let payload = b"open relay ingest";
        let encoded_object = make_encoded_object(payload, tag, &key);
        let root = blake3_32(&encoded_object);
        let shards = object_to_shards(&encoded_object, Namespace(11), Epoch(46), tag, root)
            .expect("sharding should succeed");
        let bytes = encode_shard_cbor(&shards[0]).expect("shard should encode");

        let mut adapter = InMemoryAdapter::default();
        adapter.enqueue_inbound("peer-any", bytes);
        let peers = vec!["peer-any".to_string()];
        let mut stats = RuntimeStats::default();
        let mut cfg = NodeRuntimeConfig::default();
        cfg.accept_all_tags = true;

        let event = pump_once_with_config(
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

        assert!(event.is_some());
        assert!(!matches!(
            event,
            Some(crate::receive::ReceiveEvent::IgnoredNotSubscribed)
        ));
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
