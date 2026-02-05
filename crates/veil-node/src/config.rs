use std::collections::HashMap;

use crate::ack::AckRetryPolicy;
use crate::policy::{
    fanout_for_tier as fanout_for_tier_impl, LocalWotPolicy, TrustTier, WotConfig, WotPolicy,
};
use veil_fec::profile::ErasureCodingMode;

#[derive(Debug, Clone, Copy)]
pub struct AdaptiveLaneScoringConfig {
    pub enabled: bool,
    /// Smoothing factor for EWMA updates (0..=1).
    pub ewma_alpha: f64,
    pub weight_send_success: f64,
    pub weight_ack_success: f64,
    pub weight_latency: f64,
    /// Latency normalization scale for converting p95 ms to score.
    pub latency_scale_ms: u64,
    /// If one lane underperforms by this margin, bias traffic away.
    pub hysteresis_margin: f64,
    /// Minimum fallback fanout while adaptive mode is enabled.
    pub min_fallback_fanout: usize,
}

impl Default for AdaptiveLaneScoringConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            ewma_alpha: 0.25,
            weight_send_success: 0.45,
            weight_ack_success: 0.35,
            weight_latency: 0.20,
            latency_scale_ms: 1_500,
            hysteresis_margin: 0.10,
            min_fallback_fanout: 1,
        }
    }
}

#[derive(Debug, Clone)]
pub struct NodeRuntimeConfig {
    /// Base fanout used for fast-lane forwarding.
    pub base_fast_fanout: usize,
    /// Base fanout used for fallback-lane forwarding.
    pub base_fallback_fanout: usize,
    /// Extra fallback fanout used for fast-lane redundancy sends.
    pub fallback_redundancy_fanout: usize,
    /// Cache TTL (in abstract steps) for shard entries.
    pub ttl_steps: u64,
    /// Steps to wait before first ACK-timeout retry.
    pub ack_initial_timeout_steps: u64,
    /// Number of retry shards per ACK-timeout escalation step.
    pub ack_retry_batch_size: usize,
    /// Backoff in steps between ACK retry attempts.
    pub ack_backoff_steps: u64,
    /// Maximum ACK-timeout retry attempts.
    pub ack_max_retries: u32,
    /// Global max shard entries allowed in cache.
    pub max_cache_shards: usize,
    /// Erasure coding mode used when producing/reconstructing shards.
    pub erasure_coding_mode: ErasureCodingMode,
    /// Optional upward bucket jitter levels (0 disables jitter).
    pub bucket_jitter_extra_levels: usize,
    /// Adaptive lane-scoring policy for fanout rebalancing.
    pub adaptive_lane_scoring: AdaptiveLaneScoringConfig,
    /// Local WoT policy used for trust classification and quotas.
    pub wot_policy: LocalWotPolicy,
    peer_publishers: HashMap<String, [u8; 32]>,
}

impl Default for NodeRuntimeConfig {
    fn default() -> Self {
        Self {
            base_fast_fanout: 2,
            base_fallback_fanout: 1,
            fallback_redundancy_fanout: 1,
            ttl_steps: 10_000,
            ack_initial_timeout_steps: 2,
            ack_retry_batch_size: 2,
            ack_backoff_steps: 2,
            ack_max_retries: 6,
            max_cache_shards: 100_000,
            erasure_coding_mode: ErasureCodingMode::Systematic,
            bucket_jitter_extra_levels: 0,
            adaptive_lane_scoring: AdaptiveLaneScoringConfig::default(),
            wot_policy: LocalWotPolicy::default(),
            peer_publishers: HashMap::new(),
        }
    }
}

impl NodeRuntimeConfig {
    /// Starts a fluent builder for runtime config.
    pub fn builder() -> NodeRuntimeConfigBuilder {
        NodeRuntimeConfigBuilder::default()
    }

    /// Conservative defaults for an edge-forwarder with a hot cache.
    ///
    /// Intended for publicly reachable VPS nodes that maintain many peer links
    /// and prioritize fast shard convergence without acting as an unbounded
    /// flood source.
    pub fn edge_forwarder_hot_cache_defaults() -> Self {
        let wot_cfg = WotConfig {
            trusted_forward_quota: 0.70,
            known_forward_quota: 0.22,
            unknown_forward_quota: 0.06,
            muted_forward_quota: 0.02,
            trusted_storage_budget: 120_000,
            known_storage_budget: 60_000,
            unknown_storage_budget: 20_000,
            muted_storage_budget: 2_000,
            ..WotConfig::default()
        };

        Self::builder()
            .base_fast_fanout(3)
            .base_fallback_fanout(1)
            .fallback_redundancy_fanout(1)
            .ttl_steps(15_000)
            .max_cache_shards(200_000)
            .ack_retry(2, 2, 2, 6)
            .with_wot_policy(LocalWotPolicy::new(wot_cfg))
            .build()
    }

    /// Minimal bootstrap-peer defaults focused on reliable initial discovery.
    pub fn bootstrap_peer_defaults() -> Self {
        let wot_cfg = WotConfig {
            trusted_forward_quota: 0.65,
            known_forward_quota: 0.25,
            unknown_forward_quota: 0.05,
            muted_forward_quota: 0.01,
            trusted_storage_budget: 40_000,
            known_storage_budget: 15_000,
            unknown_storage_budget: 3_000,
            muted_storage_budget: 300,
            ..WotConfig::default()
        };

        Self::builder()
            .base_fast_fanout(2)
            .base_fallback_fanout(1)
            .fallback_redundancy_fanout(1)
            .ttl_steps(8_000)
            .max_cache_shards(50_000)
            .ack_retry(2, 2, 2, 4)
            .with_wot_policy(LocalWotPolicy::new(wot_cfg))
            .build()
    }

    /// Binds a transport peer identifier to a publisher pubkey.
    pub fn bind_peer_publisher(&mut self, peer: impl Into<String>, publisher: [u8; 32]) {
        self.peer_publishers.insert(peer.into(), publisher);
    }

    /// Looks up the configured publisher pubkey for a transport peer id.
    pub fn publisher_for_peer(&self, peer: &str) -> Option<[u8; 32]> {
        self.peer_publishers.get(peer).copied()
    }

    /// Classifies an inbound peer's mapped publisher into a local trust tier.
    pub fn classify_peer_tier(&self, peer: &str, now_step: u64) -> TrustTier {
        if let Some(pubkey) = self.peer_publishers.get(peer) {
            self.wot_policy.classify_publisher(*pubkey, now_step)
        } else {
            TrustTier::Unknown
        }
    }

    /// Classifies an optional publisher pubkey into a local trust tier.
    ///
    /// This is useful for runtimes that keep peer->publisher mapping outside
    /// `NodeRuntimeConfig` and want to avoid string-based peer identifiers.
    pub fn classify_publisher_tier(&self, publisher: Option<[u8; 32]>, now_step: u64) -> TrustTier {
        publisher
            .map(|pubkey| self.wot_policy.classify_publisher(pubkey, now_step))
            .unwrap_or(TrustTier::Unknown)
    }

    /// Computes effective fanout for a peer using current WoT policy quotas.
    pub fn fanout_for_peer(&self, peer: &str, now_step: u64, base_fanout: usize) -> usize {
        let tier = self.classify_peer_tier(peer, now_step);
        fanout_for_tier_impl(base_fanout, tier, &self.wot_policy)
    }

    /// Computes effective fanout for a trust tier using current WoT quotas.
    pub fn fanout_for_tier(&self, tier: TrustTier, base_fanout: usize) -> usize {
        fanout_for_tier_impl(base_fanout, tier, &self.wot_policy)
    }

    /// Returns ACK timeout/retry policy derived from runtime config.
    pub fn ack_retry_policy(&self) -> AckRetryPolicy {
        AckRetryPolicy {
            initial_timeout_steps: self.ack_initial_timeout_steps,
            retry_batch_size: self.ack_retry_batch_size,
            backoff_step: self.ack_backoff_steps,
            max_retries: self.ack_max_retries,
        }
    }
}

/// Fluent builder for `NodeRuntimeConfig`.
#[derive(Debug, Clone, Default)]
pub struct NodeRuntimeConfigBuilder {
    cfg: NodeRuntimeConfig,
}

impl NodeRuntimeConfigBuilder {
    pub fn base_fast_fanout(mut self, value: usize) -> Self {
        self.cfg.base_fast_fanout = value;
        self
    }

    pub fn base_fallback_fanout(mut self, value: usize) -> Self {
        self.cfg.base_fallback_fanout = value;
        self
    }

    pub fn fallback_redundancy_fanout(mut self, value: usize) -> Self {
        self.cfg.fallback_redundancy_fanout = value;
        self
    }

    pub fn ttl_steps(mut self, value: u64) -> Self {
        self.cfg.ttl_steps = value;
        self
    }

    pub fn max_cache_shards(mut self, value: usize) -> Self {
        self.cfg.max_cache_shards = value;
        self
    }

    pub fn erasure_coding_mode(mut self, value: ErasureCodingMode) -> Self {
        self.cfg.erasure_coding_mode = value;
        self
    }

    pub fn bucket_jitter_extra_levels(mut self, value: usize) -> Self {
        self.cfg.bucket_jitter_extra_levels = value;
        self
    }

    pub fn ack_retry(
        mut self,
        initial_timeout_steps: u64,
        retry_batch_size: usize,
        backoff_steps: u64,
        max_retries: u32,
    ) -> Self {
        self.cfg.ack_initial_timeout_steps = initial_timeout_steps;
        self.cfg.ack_retry_batch_size = retry_batch_size;
        self.cfg.ack_backoff_steps = backoff_steps;
        self.cfg.ack_max_retries = max_retries;
        self
    }

    pub fn with_peer_publisher(mut self, peer: impl Into<String>, publisher: [u8; 32]) -> Self {
        self.cfg.bind_peer_publisher(peer, publisher);
        self
    }

    pub fn with_wot_policy(mut self, wot_policy: LocalWotPolicy) -> Self {
        self.cfg.wot_policy = wot_policy;
        self
    }

    pub fn adaptive_lane_scoring(mut self, value: AdaptiveLaneScoringConfig) -> Self {
        self.cfg.adaptive_lane_scoring = value;
        self
    }

    pub fn build(self) -> NodeRuntimeConfig {
        self.cfg
    }
}

#[cfg(test)]
mod tests {
    use super::{AdaptiveLaneScoringConfig, NodeRuntimeConfig};
    use crate::policy::TrustTier;
    use veil_fec::profile::ErasureCodingMode;

    #[test]
    fn classify_peer_uses_bound_publisher_tier() {
        let mut cfg = NodeRuntimeConfig::default();
        let pubkey = [0x42_u8; 32];
        cfg.wot_policy.block(pubkey);
        cfg.bind_peer_publisher("peer-a", pubkey);

        assert_eq!(cfg.classify_peer_tier("peer-a", 100), TrustTier::Blocked);
        assert_eq!(cfg.classify_peer_tier("peer-b", 100), TrustTier::Unknown);
    }

    #[test]
    fn fanout_for_peer_applies_wot_quota() {
        let mut cfg = NodeRuntimeConfig::default();
        let blocked_pubkey = [0x11_u8; 32];
        cfg.wot_policy.block(blocked_pubkey);
        cfg.bind_peer_publisher("peer-blocked", blocked_pubkey);

        assert_eq!(cfg.fanout_for_peer("peer-blocked", 0, 10), 0);
        assert!(cfg.fanout_for_peer("peer-unknown", 0, 10) > 0);
    }

    #[test]
    fn ack_retry_policy_reflects_config_fields() {
        let cfg = NodeRuntimeConfig {
            ack_initial_timeout_steps: 5,
            ack_retry_batch_size: 3,
            ack_backoff_steps: 7,
            ack_max_retries: 9,
            ..NodeRuntimeConfig::default()
        };

        let p = cfg.ack_retry_policy();
        assert_eq!(p.initial_timeout_steps, 5);
        assert_eq!(p.retry_batch_size, 3);
        assert_eq!(p.backoff_step, 7);
        assert_eq!(p.max_retries, 9);
    }

    #[test]
    fn builder_sets_selected_fields() {
        let cfg = NodeRuntimeConfig::builder()
            .base_fast_fanout(5)
            .base_fallback_fanout(2)
            .ttl_steps(42)
            .ack_retry(3, 4, 5, 6)
            .max_cache_shards(77)
            .erasure_coding_mode(ErasureCodingMode::HardenedNonSystematic)
            .bucket_jitter_extra_levels(1)
            .adaptive_lane_scoring(AdaptiveLaneScoringConfig {
                enabled: true,
                ..AdaptiveLaneScoringConfig::default()
            })
            .with_peer_publisher("peer-a", [0x99; 32])
            .build();

        assert_eq!(cfg.base_fast_fanout, 5);
        assert_eq!(cfg.base_fallback_fanout, 2);
        assert_eq!(cfg.ttl_steps, 42);
        assert_eq!(cfg.max_cache_shards, 77);
        assert_eq!(
            cfg.erasure_coding_mode,
            ErasureCodingMode::HardenedNonSystematic
        );
        assert_eq!(cfg.bucket_jitter_extra_levels, 1);
        assert!(cfg.adaptive_lane_scoring.enabled);
        assert_eq!(cfg.classify_peer_tier("peer-a", 0), TrustTier::Unknown);
        let p = cfg.ack_retry_policy();
        assert_eq!(p.initial_timeout_steps, 3);
        assert_eq!(p.retry_batch_size, 4);
        assert_eq!(p.backoff_step, 5);
        assert_eq!(p.max_retries, 6);
    }

    #[test]
    fn profile_defaults_are_conservative_and_nonzero() {
        let edge = NodeRuntimeConfig::edge_forwarder_hot_cache_defaults();
        assert!(edge.base_fast_fanout >= 2);
        assert!(edge.max_cache_shards >= 100_000);
        assert!(edge.wot_policy.config.unknown_forward_quota <= 0.10);

        let bootstrap = NodeRuntimeConfig::bootstrap_peer_defaults();
        assert!(bootstrap.base_fast_fanout <= edge.base_fast_fanout);
        assert!(bootstrap.max_cache_shards < edge.max_cache_shards);
        assert!(bootstrap.wot_policy.config.unknown_forward_quota <= 0.10);
    }
}
