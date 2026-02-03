use std::collections::HashMap;

use crate::ack::AckRetryPolicy;
use crate::policy::{fanout_for_tier, LocalWotPolicy, TrustTier, WotPolicy};

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
            wot_policy: LocalWotPolicy::default(),
            peer_publishers: HashMap::new(),
        }
    }
}

impl NodeRuntimeConfig {
    /// Binds a transport peer identifier to a publisher pubkey.
    pub fn bind_peer_publisher(&mut self, peer: impl Into<String>, publisher: [u8; 32]) {
        self.peer_publishers.insert(peer.into(), publisher);
    }

    /// Classifies an inbound peer's mapped publisher into a local trust tier.
    pub fn classify_peer_tier(&self, peer: &str, now_step: u64) -> TrustTier {
        if let Some(pubkey) = self.peer_publishers.get(peer) {
            self.wot_policy.classify_publisher(*pubkey, now_step)
        } else {
            TrustTier::Unknown
        }
    }

    /// Computes effective fanout for a peer using current WoT policy quotas.
    pub fn fanout_for_peer(&self, peer: &str, now_step: u64, base_fanout: usize) -> usize {
        let tier = self.classify_peer_tier(peer, now_step);
        fanout_for_tier(base_fanout, tier, &self.wot_policy)
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

#[cfg(test)]
mod tests {
    use super::NodeRuntimeConfig;
    use crate::policy::TrustTier;

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
}
