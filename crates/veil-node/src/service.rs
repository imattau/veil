use std::time::Duration;
use veil_core::{Epoch, Namespace, Tag};
use veil_crypto::aead::AeadCipher;
use veil_crypto::signing::{Signer, Verifier};
use veil_transport::adapter::{TransportAdapter, TransportHealthSnapshot};

use crate::batch::FeedBatcher;
use crate::bloom::{encode_bloom_exchange_packet, BloomFilter};
use crate::config::{AdaptiveLaneScoringConfig, NodeRuntimeConfig};
use crate::policy::EndorsementIngestResult;
use crate::publish::{
    publish_service_tick_multi_lane, PublishError, PublishOptions, PublishQueueTickParams,
    PublishServiceTickParams, PublishServiceTickResult,
};
use crate::receive::{ReceiveError, ReceiveEvent};
use crate::runtime::{
    pump_multi_lane_tick_with_config_split, ConfigMultiLanePumpParams, RuntimeStats,
};
use crate::state::NodeState;

/// Inputs used by one publisher runtime tick.
#[derive(Debug, Clone, Copy)]
pub struct PublisherTickInput<'a, PFast, PFallback> {
    pub namespace: Namespace,
    pub epoch: Epoch,
    pub tag: Tag,
    pub now_step: u64,
    pub flags: u16,
    pub interactive_flush: bool,
    pub fast_peers: &'a [PFast],
    pub fallback_peers: &'a [PFallback],
}

/// Typed publisher tick input using `PublishOptions` instead of raw bitflags.
#[derive(Debug, Clone, Copy)]
pub struct PublisherTickOptionsInput<'a, PFast, PFallback> {
    pub namespace: Namespace,
    pub epoch: Epoch,
    pub tag: Tag,
    pub now_step: u64,
    pub options: PublishOptions,
    pub interactive_flush: bool,
    pub fast_peers: &'a [PFast],
    pub fallback_peers: &'a [PFallback],
}

/// Optional callbacks fired after one node runtime tick.
pub type DeliveredCallback<'a> = dyn FnMut(veil_core::ObjectRoot, &[u8]) + 'a;
pub type CountCallback<'a> = dyn FnMut(usize) + 'a;

#[derive(Default)]
pub struct NodeRuntimeCallbacks<'a> {
    pub on_delivered: Option<&'a mut DeliveredCallback<'a>>,
    pub on_ack_cleared: Option<&'a mut CountCallback<'a>>,
    pub on_send_failure: Option<&'a mut CountCallback<'a>>,
    pub on_endorsement_ingested: Option<&'a mut CountCallback<'a>>,
}

/// Aggregated per-lane transport health snapshots for a node runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct NodeRuntimeTransportHealth {
    pub fast_lane: TransportHealthSnapshot,
    pub fallback_lane: TransportHealthSnapshot,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AdaptiveLaneScoreSnapshot {
    pub fast_score: f64,
    pub fallback_score: f64,
    pub effective_fast_fanout: usize,
    pub effective_fallback_fanout: usize,
}

#[derive(Debug, Clone, Copy)]
struct AdaptiveLaneScoringState {
    fast_score: f64,
    fallback_score: f64,
    fast_send_success_ewma: f64,
    fallback_send_success_ewma: f64,
    fast_ack_ewma: f64,
    fallback_ack_ewma: f64,
    last_fast_snapshot: TransportHealthSnapshot,
    last_fallback_snapshot: TransportHealthSnapshot,
    effective_fast_fanout: usize,
    effective_fallback_fanout: usize,
}

impl AdaptiveLaneScoringState {
    fn new(base_fast: usize, base_fallback: usize) -> Self {
        Self {
            fast_score: 0.5,
            fallback_score: 0.5,
            fast_send_success_ewma: 0.8,
            fallback_send_success_ewma: 0.8,
            fast_ack_ewma: 0.5,
            fallback_ack_ewma: 0.5,
            last_fast_snapshot: TransportHealthSnapshot::default(),
            last_fallback_snapshot: TransportHealthSnapshot::default(),
            effective_fast_fanout: base_fast,
            effective_fallback_fanout: base_fallback,
        }
    }
}

/// Runtime loop configuration for `NodeRuntime` orchestration.
#[derive(Debug, Clone, Copy)]
pub struct NodeRuntimeRunnerConfig {
    /// Initial step value passed into the first `tick`.
    pub start_step: u64,
    /// Delay between successful ticks.
    pub tick_interval: Duration,
    /// Delay applied after an error before next attempt.
    pub error_backoff: Duration,
    /// If set, exits loop after this many consecutive tick errors.
    pub max_consecutive_errors: Option<u32>,
}

impl Default for NodeRuntimeRunnerConfig {
    fn default() -> Self {
        Self {
            start_step: 0,
            tick_interval: Duration::from_millis(50),
            error_backoff: Duration::from_millis(250),
            max_consecutive_errors: Some(32),
        }
    }
}

fn clamp01(value: f64) -> f64 {
    value.clamp(0.0, 1.0)
}

fn ewma_update(previous: f64, next: f64, alpha: f64) -> f64 {
    let a = alpha.clamp(0.0, 1.0);
    (1.0 - a) * previous + a * next
}

fn ratio_or_neutral(ok: u64, total: u64, neutral: f64) -> f64 {
    if total == 0 {
        neutral
    } else {
        ok as f64 / total as f64
    }
}

fn latency_to_score(p95_latency_ms: Option<u64>, scale_ms: u64) -> f64 {
    let Some(p95) = p95_latency_ms else {
        return 0.5;
    };
    let scale = scale_ms.max(1) as f64;
    clamp01(1.0 - (p95 as f64 / scale))
}

fn send_delta(previous: TransportHealthSnapshot, current: TransportHealthSnapshot) -> (u64, u64) {
    let ok = current
        .outbound_send_ok
        .saturating_sub(previous.outbound_send_ok);
    let err = current
        .outbound_send_err
        .saturating_sub(previous.outbound_send_err);
    (ok, ok.saturating_add(err))
}

fn rebalance_fanout(
    base_fast: usize,
    base_fallback: usize,
    fast_score: f64,
    fallback_score: f64,
    cfg: AdaptiveLaneScoringConfig,
) -> (usize, usize) {
    let total = base_fast.saturating_add(base_fallback).max(1);
    let mut fast = base_fast;
    let mut fallback = base_fallback.max(cfg.min_fallback_fanout.min(total));

    let score_sum = (fast_score + fallback_score).max(0.000_1);
    let gap = fast_score - fallback_score;
    if gap.abs() >= cfg.hysteresis_margin {
        fast = ((total as f64) * (fast_score / score_sum)).round() as usize;
        fast = fast.min(total);
        fallback = total.saturating_sub(fast);
        fallback = fallback.max(cfg.min_fallback_fanout.min(total));
        fast = total.saturating_sub(fallback);
    }
    (fast, fallback)
}

/// Exit reason for `NodeRuntime` loop helpers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeRuntimeRunnerExit {
    /// Loop ended because cancellation callback returned `true`.
    Cancelled { steps: u64 },
    /// Loop ended because requested step budget was fully consumed.
    Completed { steps: u64 },
    /// Loop ended because max consecutive errors threshold was reached.
    MaxConsecutiveErrors { steps: u64, consecutive_errors: u32 },
}

/// Stateful node runtime facade around `pump_multi_lane_tick_with_config_split`.
///
/// This reduces call-site wiring by owning state, adapters, crypto handles,
/// key material, config, and stats.
pub struct NodeRuntime<AFast, AFallback, C, V>
where
    AFast: TransportAdapter,
    AFallback: TransportAdapter,
    C: AeadCipher,
    V: Verifier,
{
    pub state: NodeState,
    pub fast_adapter: AFast,
    pub fallback_adapter: AFallback,
    pub config: NodeRuntimeConfig,
    pub decrypt_key: [u8; 32],
    pub stats: RuntimeStats,
    adaptive_lane_state: AdaptiveLaneScoringState,
    last_bloom_exchange_step: Option<u64>,
    cipher: C,
    verifier: V,
}

impl<AFast, AFallback, C, V> NodeRuntime<AFast, AFallback, C, V>
where
    AFast: TransportAdapter,
    AFallback: TransportAdapter,
    C: AeadCipher,
    V: Verifier,
{
    pub fn new(
        state: NodeState,
        fast_adapter: AFast,
        fallback_adapter: AFallback,
        config: NodeRuntimeConfig,
        decrypt_key: [u8; 32],
        cipher: C,
        verifier: V,
    ) -> Self {
        let adaptive_lane_state =
            AdaptiveLaneScoringState::new(config.base_fast_fanout, config.base_fallback_fanout);
        Self {
            state,
            fast_adapter,
            fallback_adapter,
            config,
            decrypt_key,
            stats: RuntimeStats::default(),
            adaptive_lane_state,
            last_bloom_exchange_step: None,
            cipher,
            verifier,
        }
    }

    /// Returns transport health counters for both lanes.
    pub fn transport_health(&self) -> NodeRuntimeTransportHealth {
        NodeRuntimeTransportHealth {
            fast_lane: self.fast_adapter.health_snapshot(),
            fallback_lane: self.fallback_adapter.health_snapshot(),
        }
    }

    pub fn adaptive_lane_scores(&self) -> Option<AdaptiveLaneScoreSnapshot> {
        if !self.config.adaptive_lane_scoring.enabled {
            return None;
        }
        Some(AdaptiveLaneScoreSnapshot {
            fast_score: self.adaptive_lane_state.fast_score,
            fallback_score: self.adaptive_lane_state.fallback_score,
            effective_fast_fanout: self.adaptive_lane_state.effective_fast_fanout,
            effective_fallback_fanout: self.adaptive_lane_state.effective_fallback_fanout,
        })
    }

    fn effective_lane_fanouts(&self) -> (usize, usize) {
        if self.config.adaptive_lane_scoring.enabled {
            (
                self.adaptive_lane_state.effective_fast_fanout,
                self.adaptive_lane_state.effective_fallback_fanout,
            )
        } else {
            (
                self.config.base_fast_fanout,
                self.config.base_fallback_fanout,
            )
        }
    }

    fn update_adaptive_lane_scoring(&mut self, ack_delta: usize) {
        let cfg = self.config.adaptive_lane_scoring;
        if !cfg.enabled {
            return;
        }
        let fast = self.fast_adapter.health_snapshot();
        let fallback = self.fallback_adapter.health_snapshot();
        let fast_latency = self.fast_adapter.p95_latency_ms();
        let fallback_latency = self.fallback_adapter.p95_latency_ms();
        let fast_ack = self.fast_adapter.ack_success_rate();
        let fallback_ack = self.fallback_adapter.ack_success_rate();

        let (fast_send_ok, fast_send_total) =
            send_delta(self.adaptive_lane_state.last_fast_snapshot, fast);
        let (fallback_send_ok, fallback_send_total) =
            send_delta(self.adaptive_lane_state.last_fallback_snapshot, fallback);

        self.adaptive_lane_state.fast_send_success_ewma = ewma_update(
            self.adaptive_lane_state.fast_send_success_ewma,
            ratio_or_neutral(fast_send_ok, fast_send_total, 0.5),
            cfg.ewma_alpha,
        );
        self.adaptive_lane_state.fallback_send_success_ewma = ewma_update(
            self.adaptive_lane_state.fallback_send_success_ewma,
            ratio_or_neutral(fallback_send_ok, fallback_send_total, 0.5),
            cfg.ewma_alpha,
        );

        let ack_hint = if ack_delta > 0 { 0.8 } else { 0.5 };
        self.adaptive_lane_state.fast_ack_ewma = ewma_update(
            self.adaptive_lane_state.fast_ack_ewma,
            fast_ack.unwrap_or(ack_hint),
            cfg.ewma_alpha,
        );
        self.adaptive_lane_state.fallback_ack_ewma = ewma_update(
            self.adaptive_lane_state.fallback_ack_ewma,
            fallback_ack.unwrap_or(ack_hint),
            cfg.ewma_alpha,
        );

        let fast_latency_score = latency_to_score(fast_latency, cfg.latency_scale_ms);
        let fallback_latency_score = latency_to_score(fallback_latency, cfg.latency_scale_ms);

        self.adaptive_lane_state.fast_score = clamp01(
            cfg.weight_send_success * self.adaptive_lane_state.fast_send_success_ewma
                + cfg.weight_ack_success * self.adaptive_lane_state.fast_ack_ewma
                + cfg.weight_latency * fast_latency_score,
        );
        self.adaptive_lane_state.fallback_score = clamp01(
            cfg.weight_send_success * self.adaptive_lane_state.fallback_send_success_ewma
                + cfg.weight_ack_success * self.adaptive_lane_state.fallback_ack_ewma
                + cfg.weight_latency * fallback_latency_score,
        );

        let (fast_fanout, fallback_fanout) = rebalance_fanout(
            self.config.base_fast_fanout,
            self.config.base_fallback_fanout,
            self.adaptive_lane_state.fast_score,
            self.adaptive_lane_state.fallback_score,
            cfg,
        );
        self.adaptive_lane_state.effective_fast_fanout = fast_fanout;
        self.adaptive_lane_state.effective_fallback_fanout = fallback_fanout;
        self.adaptive_lane_state.last_fast_snapshot = fast;
        self.adaptive_lane_state.last_fallback_snapshot = fallback;
    }

    fn maybe_broadcast_bloom_filters(
        &mut self,
        now_step: u64,
        fast_peers: &[AFast::Peer],
        fallback_peers: &[AFallback::Peer],
    ) {
        let cfg = self.config.bloom_exchange;
        if !cfg.enabled || cfg.interval_steps == 0 {
            return;
        }
        if self
            .last_bloom_exchange_step
            .is_some_and(|prev| now_step.saturating_sub(prev) < cfg.interval_steps)
        {
            return;
        }

        let mut salt = [0_u8; 16];
        salt[..8].copy_from_slice(&now_step.to_be_bytes());
        let mut bf =
            BloomFilter::recommended(self.state.cache.len(), cfg.false_positive_rate, salt);
        for sid in self.state.cache.keys() {
            bf.insert(sid);
        }
        let Ok(packet) = encode_bloom_exchange_packet(now_step as u32, bf) else {
            return;
        };

        for peer in fast_peers {
            if self.fast_adapter.send(peer, &packet).is_ok() {
                self.stats.forwarded_messages += 1;
            } else {
                self.stats.send_failures += 1;
            }
        }
        for peer in fallback_peers {
            if self.fallback_adapter.send(peer, &packet).is_ok() {
                self.stats.forwarded_messages += 1;
            } else {
                self.stats.send_failures += 1;
            }
        }
        self.last_bloom_exchange_step = Some(now_step);
    }

    pub fn tick(
        &mut self,
        now_step: u64,
        fast_peers: &[AFast::Peer],
        fallback_peers: &[AFallback::Peer],
    ) -> Result<Option<ReceiveEvent>, ReceiveError>
    where
        AFast::Peer: ToString,
        AFallback::Peer: ToString,
    {
        let (effective_fast_fanout, effective_fallback_fanout) = self.effective_lane_fanouts();
        let mut cfg = self.config.clone();
        cfg.base_fast_fanout = effective_fast_fanout;
        cfg.base_fallback_fanout = effective_fallback_fanout;

        let prev_ack = self.stats.ack_messages;
        let result = pump_multi_lane_tick_with_config_split(
            &mut self.state,
            &mut self.fast_adapter,
            &mut self.fallback_adapter,
            ConfigMultiLanePumpParams {
                fast_peers,
                fallback_peers,
                now_step,
                decrypt_key: &self.decrypt_key,
                config: &cfg,
                stats: &mut self.stats,
            },
            &self.cipher,
            &self.verifier,
        );
        if result.is_ok() {
            let ack_delta = self.stats.ack_messages.saturating_sub(prev_ack);
            self.update_adaptive_lane_scoring(ack_delta);
            self.maybe_broadcast_bloom_filters(now_step, fast_peers, fallback_peers);
        }
        result
    }

    pub fn tick_with_callbacks(
        &mut self,
        now_step: u64,
        fast_peers: &[AFast::Peer],
        fallback_peers: &[AFallback::Peer],
        callbacks: NodeRuntimeCallbacks<'_>,
    ) -> Result<Option<ReceiveEvent>, ReceiveError>
    where
        AFast::Peer: ToString,
        AFallback::Peer: ToString,
    {
        let mut callbacks = callbacks;
        self.tick_with_callbacks_ref(now_step, fast_peers, fallback_peers, &mut callbacks)
    }

    pub fn tick_with_callbacks_ref(
        &mut self,
        now_step: u64,
        fast_peers: &[AFast::Peer],
        fallback_peers: &[AFallback::Peer],
        callbacks: &mut NodeRuntimeCallbacks<'_>,
    ) -> Result<Option<ReceiveEvent>, ReceiveError>
    where
        AFast::Peer: ToString,
        AFallback::Peer: ToString,
    {
        let prev_ack = self.stats.ack_messages;
        let prev_fail = self.stats.send_failures;
        let mut endorsement_delta = 0usize;
        let event = self.tick(now_step, fast_peers, fallback_peers)?;

        if let Some(ReceiveEvent::Delivered {
            object_root,
            payload,
            ..
        }) = event.as_ref()
        {
            if let Some(endorsement) = crate::policy::parse_endorsement_payload(payload) {
                let outcome = self.config.wot_policy.ingest_endorsement(
                    endorsement.endorser,
                    endorsement.publisher,
                    endorsement.at_step,
                    now_step,
                );
                if outcome == EndorsementIngestResult::Applied {
                    endorsement_delta += 1;
                }
            }
            if let Some(cb) = callbacks.on_delivered.as_mut() {
                (*cb)(*object_root, payload);
            }
        }

        let ack_delta = self.stats.ack_messages.saturating_sub(prev_ack);
        if ack_delta > 0 {
            if let Some(cb) = callbacks.on_ack_cleared.as_mut() {
                (*cb)(ack_delta);
            }
        }

        let fail_delta = self.stats.send_failures.saturating_sub(prev_fail);
        if fail_delta > 0 {
            if let Some(cb) = callbacks.on_send_failure.as_mut() {
                (*cb)(fail_delta);
            }
        }
        if endorsement_delta > 0 {
            if let Some(cb) = callbacks.on_endorsement_ingested.as_mut() {
                (*cb)(endorsement_delta);
            }
        }

        Ok(event)
    }

    /// Runs ticks until cancellation callback returns true.
    pub fn run_until<F>(
        &mut self,
        fast_peers: &[AFast::Peer],
        fallback_peers: &[AFallback::Peer],
        config: NodeRuntimeRunnerConfig,
        mut should_stop: F,
        mut callbacks: Option<&mut NodeRuntimeCallbacks<'_>>,
    ) -> NodeRuntimeRunnerExit
    where
        AFast::Peer: ToString,
        AFallback::Peer: ToString,
        F: FnMut() -> bool,
    {
        let mut step = config.start_step;
        let mut steps = 0_u64;
        let mut consecutive_errors = 0_u32;

        loop {
            if should_stop() {
                return NodeRuntimeRunnerExit::Cancelled { steps };
            }

            let tick_result = if let Some(cb) = callbacks.as_deref_mut() {
                self.tick_with_callbacks_ref(step, fast_peers, fallback_peers, cb)
            } else {
                self.tick(step, fast_peers, fallback_peers)
            };

            match tick_result {
                Ok(_) => {
                    consecutive_errors = 0;
                    if !config.tick_interval.is_zero() {
                        std::thread::sleep(config.tick_interval);
                    }
                }
                Err(_) => {
                    consecutive_errors = consecutive_errors.saturating_add(1);
                    if let Some(max) = config.max_consecutive_errors {
                        if consecutive_errors >= max {
                            return NodeRuntimeRunnerExit::MaxConsecutiveErrors {
                                steps,
                                consecutive_errors,
                            };
                        }
                    }
                    if !config.error_backoff.is_zero() {
                        std::thread::sleep(config.error_backoff);
                    }
                }
            }

            step = step.saturating_add(1);
            steps = steps.saturating_add(1);
        }
    }

    /// Runs a fixed number of ticks.
    pub fn run_steps(
        &mut self,
        steps: u64,
        fast_peers: &[AFast::Peer],
        fallback_peers: &[AFallback::Peer],
        config: NodeRuntimeRunnerConfig,
        mut callbacks: Option<&mut NodeRuntimeCallbacks<'_>>,
    ) -> NodeRuntimeRunnerExit
    where
        AFast::Peer: ToString,
        AFallback::Peer: ToString,
    {
        let mut step = config.start_step;
        let mut ran = 0_u64;
        let mut consecutive_errors = 0_u32;

        while ran < steps {
            let tick_result = if let Some(cb) = callbacks.as_deref_mut() {
                self.tick_with_callbacks_ref(step, fast_peers, fallback_peers, cb)
            } else {
                self.tick(step, fast_peers, fallback_peers)
            };

            match tick_result {
                Ok(_) => {
                    consecutive_errors = 0;
                    if !config.tick_interval.is_zero() {
                        std::thread::sleep(config.tick_interval);
                    }
                }
                Err(_) => {
                    consecutive_errors = consecutive_errors.saturating_add(1);
                    if let Some(max) = config.max_consecutive_errors {
                        if consecutive_errors >= max {
                            return NodeRuntimeRunnerExit::MaxConsecutiveErrors {
                                steps: ran,
                                consecutive_errors,
                            };
                        }
                    }
                    if !config.error_backoff.is_zero() {
                        std::thread::sleep(config.error_backoff);
                    }
                }
            }

            ran = ran.saturating_add(1);
            step = step.saturating_add(1);
        }

        NodeRuntimeRunnerExit::Completed { steps: ran }
    }
}

/// Stateful publisher runtime facade around `publish_service_tick_multi_lane`.
///
/// This owns queue/batcher, runtime state, adapters, config, key material, and
/// optional signer for one-call publish ticks.
pub struct PublisherRuntime<AFast, AFallback, C, S>
where
    AFast: TransportAdapter,
    AFallback: TransportAdapter,
    C: AeadCipher,
    S: Signer,
{
    pub state: NodeState,
    pub batcher: FeedBatcher,
    pub fast_adapter: AFast,
    pub fallback_adapter: AFallback,
    pub config: NodeRuntimeConfig,
    pub encrypt_key: [u8; 32],
    pub signer: Option<S>,
    cipher: C,
}

impl<AFast, AFallback, C, S> PublisherRuntime<AFast, AFallback, C, S>
where
    AFast: TransportAdapter,
    AFallback: TransportAdapter,
    C: AeadCipher,
    S: Signer,
{
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        state: NodeState,
        batcher: FeedBatcher,
        fast_adapter: AFast,
        fallback_adapter: AFallback,
        config: NodeRuntimeConfig,
        encrypt_key: [u8; 32],
        signer: Option<S>,
        cipher: C,
    ) -> Self {
        Self {
            state,
            batcher,
            fast_adapter,
            fallback_adapter,
            config,
            encrypt_key,
            signer,
            cipher,
        }
    }

    pub fn enqueue(&mut self, item: Vec<u8>) {
        self.batcher.enqueue(item);
    }

    pub fn tick(
        &mut self,
        input: PublisherTickInput<'_, AFast::Peer, AFallback::Peer>,
    ) -> Result<PublishServiceTickResult, PublishError> {
        publish_service_tick_multi_lane(
            &mut self.state,
            &mut self.fast_adapter,
            &mut self.fallback_adapter,
            PublishServiceTickParams {
                batcher: &mut self.batcher,
                publish: PublishQueueTickParams {
                    namespace: input.namespace,
                    epoch: input.epoch,
                    tag: input.tag,
                    encrypt_key: &self.encrypt_key,
                    now_step: input.now_step,
                    flags: input.flags,
                    interactive_flush: input.interactive_flush,
                    fast_peers: input.fast_peers,
                    fallback_peers: input.fallback_peers,
                },
            },
            &self.config,
            &self.cipher,
            self.signer.as_ref(),
        )
    }

    pub fn tick_with_options(
        &mut self,
        input: PublisherTickOptionsInput<'_, AFast::Peer, AFallback::Peer>,
    ) -> Result<PublishServiceTickResult, PublishError> {
        self.tick(PublisherTickInput {
            namespace: input.namespace,
            epoch: input.epoch,
            tag: input.tag,
            now_step: input.now_step,
            flags: input.options.to_flags(),
            interactive_flush: input.interactive_flush,
            fast_peers: input.fast_peers,
            fallback_peers: input.fallback_peers,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use crate::bloom::decode_bloom_exchange_packet;
    use crate::config::BloomExchangeConfig;
    use veil_codec::object::OBJECT_FLAG_SIGNED;
    use veil_codec::shard::encode_shard_cbor;
    use veil_core::{Epoch, Namespace};
    use veil_crypto::aead::XChaCha20Poly1305Cipher;
    use veil_crypto::signing::{Ed25519Signer, Ed25519Verifier};
    use veil_fec::sharder::{derive_object_root, object_to_shards};
    use veil_transport::adapter::{CappedInMemoryAdapter, InMemoryAdapter};

    use super::{
        NodeRuntime, NodeRuntimeCallbacks, NodeRuntimeRunnerConfig, NodeRuntimeRunnerExit,
        NodeRuntimeTransportHealth, PublisherRuntime, PublisherTickInput,
        PublisherTickOptionsInput,
    };

    #[test]
    fn publisher_runtime_tick_publishes_from_queue() {
        let mut rt = PublisherRuntime::new(
            crate::state::NodeState::default(),
            crate::batch::FeedBatcher::default(),
            InMemoryAdapter::default(),
            InMemoryAdapter::default(),
            crate::config::NodeRuntimeConfig::default(),
            [0xAA; 32],
            Some(Ed25519Signer::from_secret([0x11; 32])),
            XChaCha20Poly1305Cipher,
        );
        rt.enqueue(b"hello".to_vec());
        let peers = vec!["peer-a".to_string(), "peer-b".to_string()];

        let out = rt
            .tick(PublisherTickInput {
                namespace: veil_core::Namespace(1),
                epoch: veil_core::Epoch(1),
                tag: [0x22; 32],
                now_step: 1,
                flags: OBJECT_FLAG_SIGNED,
                interactive_flush: true,
                fast_peers: &peers,
                fallback_peers: &peers,
            })
            .expect("tick should succeed");

        assert!(out.published.is_some());
    }

    #[test]
    fn node_runtime_tick_runs_without_message() {
        let mut rt = NodeRuntime::new(
            crate::state::NodeState::default(),
            InMemoryAdapter::default(),
            InMemoryAdapter::default(),
            crate::config::NodeRuntimeConfig::default(),
            [0xAA; 32],
            XChaCha20Poly1305Cipher,
            Ed25519Verifier,
        );
        let peers = vec!["peer-a".to_string()];

        let out = rt.tick(1, &peers, &peers).expect("tick should succeed");
        assert!(out.is_none());
    }

    #[test]
    fn node_runtime_tick_broadcasts_bloom_packets_when_enabled() {
        let mut rt = NodeRuntime::new(
            crate::state::NodeState::default(),
            InMemoryAdapter::default(),
            InMemoryAdapter::default(),
            crate::config::NodeRuntimeConfig::builder()
                .bloom_exchange(BloomExchangeConfig {
                    enabled: true,
                    interval_steps: 1,
                    false_positive_rate: 0.05,
                })
                .build(),
            [0xAA; 32],
            XChaCha20Poly1305Cipher,
            Ed25519Verifier,
        );
        let peers = vec!["peer-a".to_string(), "peer-b".to_string()];

        let _ = rt.tick(1, &peers, &peers).expect("tick should succeed");
        let fast = rt.fast_adapter.take_outbound();
        let fallback = rt.fallback_adapter.take_outbound();
        assert!(!fast.is_empty());
        assert!(!fallback.is_empty());
        assert!(decode_bloom_exchange_packet(&fast[0].1).is_some());
        assert!(decode_bloom_exchange_packet(&fallback[0].1).is_some());
    }

    #[test]
    fn bloom_exchange_interval_estimates_control_plane_traffic_overhead() {
        let peers = vec!["peer-a".to_string(), "peer-b".to_string()];
        let mut rt_off = NodeRuntime::new(
            crate::state::NodeState::default(),
            InMemoryAdapter::default(),
            InMemoryAdapter::default(),
            crate::config::NodeRuntimeConfig::default(),
            [0xAA; 32],
            XChaCha20Poly1305Cipher,
            Ed25519Verifier,
        );
        let mut rt_on = NodeRuntime::new(
            crate::state::NodeState::default(),
            InMemoryAdapter::default(),
            InMemoryAdapter::default(),
            crate::config::NodeRuntimeConfig::builder()
                .bloom_exchange(BloomExchangeConfig {
                    enabled: true,
                    interval_steps: 2,
                    false_positive_rate: 0.05,
                })
                .build(),
            [0xAA; 32],
            XChaCha20Poly1305Cipher,
            Ed25519Verifier,
        );

        for step in 1..=6 {
            let _ = rt_off
                .tick(step, &peers, &peers)
                .expect("tick off should succeed");
            let _ = rt_on
                .tick(step, &peers, &peers)
                .expect("tick on should succeed");
        }

        let off_fast = rt_off.fast_adapter.take_outbound();
        let off_fallback = rt_off.fallback_adapter.take_outbound();
        let on_fast = rt_on.fast_adapter.take_outbound();
        let on_fallback = rt_on.fallback_adapter.take_outbound();

        assert!(off_fast.is_empty());
        assert!(off_fallback.is_empty());

        let bloom_packets = on_fast
            .iter()
            .chain(on_fallback.iter())
            .filter(|(_, bytes)| decode_bloom_exchange_packet(bytes).is_some())
            .count();

        eprintln!("bloom exchange traffic estimate: bloom_packets={bloom_packets}");
        // Steps 1,3,5 each broadcast to 2 fast + 2 fallback peers.
        assert_eq!(bloom_packets, 12);
        // A simple traffic estimate signal: enabling bloom added 12 control sends.
        assert_eq!(
            rt_on.stats.forwarded_messages - rt_off.stats.forwarded_messages,
            12
        );
    }

    #[test]
    fn node_runtime_exposes_transport_health_snapshots() {
        let rt = NodeRuntime::new(
            crate::state::NodeState::default(),
            InMemoryAdapter::default(),
            InMemoryAdapter::default(),
            crate::config::NodeRuntimeConfig::default(),
            [0xAA; 32],
            XChaCha20Poly1305Cipher,
            Ed25519Verifier,
        );

        assert_eq!(rt.transport_health(), NodeRuntimeTransportHealth::default());
    }

    #[test]
    fn publisher_runtime_tick_with_options_works() {
        let mut rt = PublisherRuntime::new(
            crate::state::NodeState::default(),
            crate::batch::FeedBatcher::default(),
            InMemoryAdapter::default(),
            InMemoryAdapter::default(),
            crate::config::NodeRuntimeConfig::default(),
            [0xAA; 32],
            Some(Ed25519Signer::from_secret([0x11; 32])),
            XChaCha20Poly1305Cipher,
        );
        rt.enqueue(b"hello".to_vec());
        let peers = vec!["peer-a".to_string(), "peer-b".to_string()];

        let out = rt
            .tick_with_options(PublisherTickOptionsInput {
                namespace: veil_core::Namespace(1),
                epoch: veil_core::Epoch(1),
                tag: [0x22; 32],
                now_step: 1,
                options: crate::publish::PublishOptions::signed().with_ack_requested(true),
                interactive_flush: true,
                fast_peers: &peers,
                fallback_peers: &peers,
            })
            .expect("tick should succeed");

        assert!(out.published.is_some());
    }

    #[test]
    fn node_runtime_callbacks_receive_send_failure_delta() {
        let mut rt = NodeRuntime::new(
            crate::state::NodeState::default(),
            InMemoryAdapter::default(),
            InMemoryAdapter::default(),
            crate::config::NodeRuntimeConfig::default(),
            [0xAA; 32],
            XChaCha20Poly1305Cipher,
            Ed25519Verifier,
        );
        let peers = vec!["peer-a".to_string()];
        let mut send_failure_count = 0usize;

        let _ = rt
            .tick_with_callbacks(
                1,
                &peers,
                &peers,
                NodeRuntimeCallbacks {
                    on_send_failure: Some(&mut |count| send_failure_count += count),
                    ..NodeRuntimeCallbacks::default()
                },
            )
            .expect("tick should succeed");

        assert_eq!(send_failure_count, 0);
    }

    #[test]
    fn node_runtime_run_steps_completes_requested_budget() {
        let mut rt = NodeRuntime::new(
            crate::state::NodeState::default(),
            InMemoryAdapter::default(),
            InMemoryAdapter::default(),
            crate::config::NodeRuntimeConfig::default(),
            [0xAA; 32],
            XChaCha20Poly1305Cipher,
            Ed25519Verifier,
        );
        let peers = vec!["peer-a".to_string()];
        let exit = rt.run_steps(
            5,
            &peers,
            &peers,
            NodeRuntimeRunnerConfig {
                start_step: 10,
                tick_interval: Duration::ZERO,
                error_backoff: Duration::ZERO,
                max_consecutive_errors: Some(4),
            },
            None,
        );

        assert_eq!(exit, NodeRuntimeRunnerExit::Completed { steps: 5 });
    }

    #[test]
    fn node_runtime_run_until_honors_cancellation() {
        let mut rt = NodeRuntime::new(
            crate::state::NodeState::default(),
            InMemoryAdapter::default(),
            InMemoryAdapter::default(),
            crate::config::NodeRuntimeConfig::default(),
            [0xAA; 32],
            XChaCha20Poly1305Cipher,
            Ed25519Verifier,
        );
        let peers = vec!["peer-a".to_string()];
        let mut polls = 0_u32;
        let exit = rt.run_until(
            &peers,
            &peers,
            NodeRuntimeRunnerConfig {
                start_step: 0,
                tick_interval: Duration::ZERO,
                error_backoff: Duration::ZERO,
                max_consecutive_errors: Some(4),
            },
            || {
                polls += 1;
                polls > 3
            },
            None,
        );

        assert_eq!(exit, NodeRuntimeRunnerExit::Cancelled { steps: 3 });
    }

    #[test]
    fn adaptive_lane_scoring_shifts_fanout_to_fallback_on_fast_failures() {
        let mut fast = CappedInMemoryAdapter::with_max_send_bytes(16 * 1024);
        fast.set_allow_send(false);
        let mut fallback = CappedInMemoryAdapter::with_max_send_bytes(16 * 1024);
        fallback.set_allow_send(true);

        let mut rt = NodeRuntime::new(
            crate::state::NodeState::default(),
            fast,
            fallback,
            crate::config::NodeRuntimeConfig::builder()
                .base_fast_fanout(3)
                .base_fallback_fanout(1)
                .adaptive_lane_scoring(crate::config::AdaptiveLaneScoringConfig {
                    enabled: true,
                    hysteresis_margin: 0.0,
                    min_fallback_fanout: 1,
                    ..crate::config::AdaptiveLaneScoringConfig::default()
                })
                .build(),
            [0xAA; 32],
            XChaCha20Poly1305Cipher,
            Ed25519Verifier,
        );
        let tag = [0x44; 32];
        rt.state.subscriptions.insert(tag);
        let encoded = b"adaptive lane scoring probe".to_vec();
        let root = derive_object_root(&encoded);
        let shard = object_to_shards(&encoded, Namespace(1), Epoch(1), tag, root)
            .expect("shard build should work")
            .remove(0);
        let shard_bytes = encode_shard_cbor(&shard).expect("shard should encode");

        let peers = vec![
            "peer-a".to_string(),
            "peer-b".to_string(),
            "peer-c".to_string(),
        ];
        // Drive repeated send failures on fast lane by attempting to forward.
        for _ in 0..5 {
            rt.fast_adapter
                .enqueue_inbound("origin", shard_bytes.clone());
            let _ = rt.tick(1, &peers, &peers);
        }

        let adaptive = rt
            .adaptive_lane_scores()
            .expect("adaptive scoring should be enabled");
        assert!(adaptive.effective_fallback_fanout >= 2);
        assert!(adaptive.effective_fast_fanout <= 2);
    }

    #[test]
    fn normal_social_usage_estimates_total_network_traffic() {
        let tag = [0x55; 32];
        let peers = vec![
            "peer-a".to_string(),
            "peer-b".to_string(),
            "peer-c".to_string(),
            "peer-d".to_string(),
        ];

        let mut publisher = PublisherRuntime::new(
            crate::state::NodeState::default(),
            crate::batch::FeedBatcher::default(),
            InMemoryAdapter::default(),
            InMemoryAdapter::default(),
            crate::config::NodeRuntimeConfig::default(),
            [0xAA; 32],
            Some(Ed25519Signer::from_secret([0x11; 32])),
            XChaCha20Poly1305Cipher,
        );
        let mut subscriber = NodeRuntime::new(
            crate::state::NodeState::default(),
            InMemoryAdapter::default(),
            InMemoryAdapter::default(),
            crate::config::NodeRuntimeConfig::builder()
                .bloom_exchange(BloomExchangeConfig {
                    enabled: true,
                    interval_steps: 10,
                    false_positive_rate: 0.05,
                })
                .build(),
            [0xAA; 32],
            XChaCha20Poly1305Cipher,
            Ed25519Verifier,
        );
        subscriber.state.subscriptions.insert(tag);

        // "Normal" session mix: 80 lightweight reactions + 20 post-sized payloads.
        for i in 0..100u8 {
            let item = if i % 5 == 0 {
                vec![i; 1_200]
            } else {
                vec![i; 120]
            };
            publisher.enqueue(item);
        }

        let mut total_send_ops = 0usize;
        let mut total_bytes = 0usize;
        let mut step = 1u64;

        while !publisher.batcher.is_empty() && step < 256 {
            let _ = publisher
                .tick(PublisherTickInput {
                    namespace: veil_core::types::NAMESPACE_PUBLIC_FEED,
                    epoch: veil_core::Epoch(1),
                    tag,
                    now_step: step,
                    flags: OBJECT_FLAG_SIGNED,
                    interactive_flush: false,
                    fast_peers: &peers,
                    fallback_peers: &peers,
                })
                .expect("publisher tick should succeed");

            let pub_fast = publisher.fast_adapter.take_outbound();
            let pub_fallback = publisher.fallback_adapter.take_outbound();
            total_send_ops += pub_fast.len() + pub_fallback.len();
            total_bytes += pub_fast.iter().map(|(_, b)| b.len()).sum::<usize>();
            total_bytes += pub_fallback.iter().map(|(_, b)| b.len()).sum::<usize>();

            for (_, bytes) in pub_fast {
                subscriber.fast_adapter.enqueue_inbound("publisher", bytes);
            }
            for (_, bytes) in pub_fallback {
                subscriber
                    .fallback_adapter
                    .enqueue_inbound("publisher", bytes);
            }
            step = step.saturating_add(1);
        }

        assert!(publisher.batcher.is_empty());

        for i in 0..512u64 {
            let _ = subscriber
                .tick(step + i, &peers, &peers)
                .expect("subscriber tick should succeed");
            let sub_fast = subscriber.fast_adapter.take_outbound();
            let sub_fallback = subscriber.fallback_adapter.take_outbound();
            total_send_ops += sub_fast.len() + sub_fallback.len();
            total_bytes += sub_fast.iter().map(|(_, b)| b.len()).sum::<usize>();
            total_bytes += sub_fallback.iter().map(|(_, b)| b.len()).sum::<usize>();
        }

        eprintln!(
            "normal social usage traffic estimate: sends={}, bytes={}",
            total_send_ops, total_bytes
        );

        assert!(total_send_ops > 0);
        assert!(total_bytes > 200 * 1024);
        assert!(total_bytes < 8 * 1024 * 1024);
    }

    #[test]
    fn normal_social_usage_with_100_post_reads_estimates_total_network_traffic() {
        let tag = [0x56; 32];
        let peers = vec![
            "peer-a".to_string(),
            "peer-b".to_string(),
            "peer-c".to_string(),
            "peer-d".to_string(),
        ];

        let mut publisher = PublisherRuntime::new(
            crate::state::NodeState::default(),
            crate::batch::FeedBatcher::default(),
            InMemoryAdapter::default(),
            InMemoryAdapter::default(),
            crate::config::NodeRuntimeConfig::default(),
            [0xAA; 32],
            Some(Ed25519Signer::from_secret([0x12; 32])),
            XChaCha20Poly1305Cipher,
        );
        let mut subscriber = NodeRuntime::new(
            crate::state::NodeState::default(),
            InMemoryAdapter::default(),
            InMemoryAdapter::default(),
            crate::config::NodeRuntimeConfig::builder()
                .bloom_exchange(BloomExchangeConfig {
                    enabled: true,
                    interval_steps: 10,
                    false_positive_rate: 0.05,
                })
                .build(),
            [0xAA; 32],
            XChaCha20Poly1305Cipher,
            Ed25519Verifier,
        );
        subscriber.state.subscriptions.insert(tag);

        // Baseline social usage mix.
        for i in 0..100u8 {
            let item = if i % 5 == 0 {
                vec![i; 1_200]
            } else {
                vec![i; 120]
            };
            publisher.enqueue(item);
        }

        let mut total_send_ops = 0usize;
        let mut total_bytes = 0usize;
        let mut step = 1u64;

        while !publisher.batcher.is_empty() && step < 256 {
            let _ = publisher
                .tick(PublisherTickInput {
                    namespace: veil_core::types::NAMESPACE_PUBLIC_FEED,
                    epoch: veil_core::Epoch(1),
                    tag,
                    now_step: step,
                    flags: OBJECT_FLAG_SIGNED,
                    interactive_flush: false,
                    fast_peers: &peers,
                    fallback_peers: &peers,
                })
                .expect("publisher tick should succeed");

            let pub_fast = publisher.fast_adapter.take_outbound();
            let pub_fallback = publisher.fallback_adapter.take_outbound();
            total_send_ops += pub_fast.len() + pub_fallback.len();
            total_bytes += pub_fast.iter().map(|(_, b)| b.len()).sum::<usize>();
            total_bytes += pub_fallback.iter().map(|(_, b)| b.len()).sum::<usize>();

            for (_, bytes) in pub_fast {
                subscriber.fast_adapter.enqueue_inbound("publisher", bytes);
            }
            for (_, bytes) in pub_fallback {
                subscriber
                    .fallback_adapter
                    .enqueue_inbound("publisher", bytes);
            }
            step = step.saturating_add(1);
        }
        assert!(publisher.batcher.is_empty());

        for i in 0..512u64 {
            let _ = subscriber
                .tick(step + i, &peers, &peers)
                .expect("subscriber tick should succeed");
            let sub_fast = subscriber.fast_adapter.take_outbound();
            let sub_fallback = subscriber.fallback_adapter.take_outbound();
            total_send_ops += sub_fast.len() + sub_fallback.len();
            total_bytes += sub_fast.iter().map(|(_, b)| b.len()).sum::<usize>();
            total_bytes += sub_fallback.iter().map(|(_, b)| b.len()).sum::<usize>();
        }

        // Add "100 post reads" estimate:
        // - request frame per read (root + metadata) ~ 96B
        // - response: first k shard payloads for a typical post object
        let read_request_bytes = 96usize;
        let sample_post_object = vec![0xAB; 1_500];
        let sample_root = derive_object_root(&sample_post_object);
        let sample_shards = object_to_shards(
            &sample_post_object,
            veil_core::types::NAMESPACE_PUBLIC_FEED,
            Epoch(1),
            tag,
            sample_root,
        )
        .expect("sample post sharding should succeed");
        let k = sample_shards[0].header.k as usize;
        let response_bytes_per_read: usize = sample_shards
            .iter()
            .take(k)
            .map(|s| encode_shard_cbor(s).expect("encode shard").len())
            .sum();

        let reads = 100usize;
        total_send_ops += reads * (1 + k);
        total_bytes += reads * (read_request_bytes + response_bytes_per_read);

        eprintln!(
            "normal social+reads traffic estimate: sends={}, bytes={}, per_read_response_bytes={}",
            total_send_ops, total_bytes, response_bytes_per_read
        );

        assert!(total_send_ops > 600);
        assert!(total_bytes > 500 * 1024);
        assert!(total_bytes < 16 * 1024 * 1024);
    }
}
