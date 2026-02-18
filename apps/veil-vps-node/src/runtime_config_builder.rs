use std::collections::HashSet;

use tracing::info;
use veil_node::config::{
    AdaptiveLaneScoringConfig, BloomExchangeConfig, NodeRuntimeConfig,
    ProbabilisticForwardingConfig,
};

use crate::node_bootstrap::pseudo_pubkey_for_peer;

pub(super) struct RuntimeConfigInputs {
    pub max_cache_shards: usize,
    pub bucket_jitter: usize,
    pub required_signed: HashSet<u16>,
    pub adaptive_scoring: bool,
    pub probabilistic_forwarding: bool,
    pub forwarding_min_probability: f64,
    pub forwarding_replica_divisor: u64,
    pub bloom_exchange: bool,
    pub bloom_interval_steps: u64,
    pub bloom_false_positive_rate: f64,
    pub open_relay: bool,
    pub blocked_peers: Vec<String>,
}

pub(super) fn build_runtime_config(inputs: RuntimeConfigInputs) -> NodeRuntimeConfig {
    let RuntimeConfigInputs {
        max_cache_shards,
        bucket_jitter,
        required_signed,
        adaptive_scoring,
        probabilistic_forwarding,
        forwarding_min_probability,
        forwarding_replica_divisor,
        bloom_exchange,
        bloom_interval_steps,
        bloom_false_positive_rate,
        open_relay,
        blocked_peers,
    } = inputs;

    let mut cfg = NodeRuntimeConfig::edge_forwarder_hot_cache_defaults();
    cfg.max_cache_shards = max_cache_shards;
    cfg.bucket_jitter_extra_levels = bucket_jitter;
    cfg.required_signed_namespaces = required_signed;
    cfg.adaptive_lane_scoring = AdaptiveLaneScoringConfig {
        enabled: adaptive_scoring,
        ..AdaptiveLaneScoringConfig::default()
    };
    cfg.probabilistic_forwarding = ProbabilisticForwardingConfig {
        enabled: probabilistic_forwarding,
        min_probability: forwarding_min_probability.clamp(0.0, 1.0),
        replica_divisor: forwarding_replica_divisor.max(1),
    };
    cfg.bloom_exchange = BloomExchangeConfig {
        enabled: bloom_exchange,
        interval_steps: bloom_interval_steps.max(1),
        false_positive_rate: bloom_false_positive_rate.clamp(0.001, 0.5),
    };
    if open_relay {
        cfg.accept_all_tags = true;
        cfg.probabilistic_forwarding.enabled = false;
        let mut wot_cfg = cfg.wot_policy.config;
        wot_cfg.trusted_forward_quota = 1.0;
        wot_cfg.known_forward_quota = 1.0;
        wot_cfg.unknown_forward_quota = 1.0;
        wot_cfg.muted_forward_quota = 1.0;
        wot_cfg.blocked_forward_quota = 0.0;
        cfg.wot_policy.update_config(wot_cfg);
        info!("open relay mode enabled: accepting all tags and full non-blocked forwarding");
    }
    for peer in blocked_peers {
        let pseudo = pseudo_pubkey_for_peer(&peer);
        cfg.bind_peer_publisher(peer.clone(), pseudo);
        cfg.wot_policy.block(pseudo);
    }
    cfg
}

#[cfg(test)]
mod tests {
    use super::{build_runtime_config, RuntimeConfigInputs};
    use std::collections::HashSet;
    use veil_node::policy::TrustTier;

    #[test]
    fn build_runtime_config_blocks_mapped_peers() {
        let cfg = build_runtime_config(RuntimeConfigInputs {
            max_cache_shards: 123,
            bucket_jitter: 2,
            required_signed: HashSet::from([7_u16]),
            adaptive_scoring: true,
            probabilistic_forwarding: true,
            forwarding_min_probability: 0.1,
            forwarding_replica_divisor: 8,
            bloom_exchange: true,
            bloom_interval_steps: 64,
            bloom_false_positive_rate: 0.05,
            open_relay: false,
            blocked_peers: vec!["peer-a".to_string()],
        });

        assert_eq!(cfg.max_cache_shards, 123);
        assert_eq!(cfg.bucket_jitter_extra_levels, 2);
        assert!(cfg.required_signed_namespaces.contains(&7_u16));
        assert!(cfg.publisher_for_peer("peer-a").is_some());
        assert_eq!(cfg.classify_peer_tier("peer-a", 0), TrustTier::Blocked);
    }

    #[test]
    fn build_runtime_config_open_relay_enables_accept_all_tags() {
        let cfg = build_runtime_config(RuntimeConfigInputs {
            max_cache_shards: 1,
            bucket_jitter: 0,
            required_signed: HashSet::new(),
            adaptive_scoring: false,
            probabilistic_forwarding: true,
            forwarding_min_probability: 0.9,
            forwarding_replica_divisor: 1,
            bloom_exchange: false,
            bloom_interval_steps: 0,
            bloom_false_positive_rate: 0.9,
            open_relay: true,
            blocked_peers: Vec::new(),
        });

        assert!(cfg.accept_all_tags);
        assert!(!cfg.probabilistic_forwarding.enabled);
        assert_eq!(cfg.wot_policy.config.blocked_forward_quota, 0.0);
        assert_eq!(cfg.wot_policy.config.unknown_forward_quota, 1.0);
    }
}
