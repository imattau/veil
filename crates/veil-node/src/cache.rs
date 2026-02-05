use crate::policy::{ShardMeta, TrustTier, WotPolicy};
use crate::state::{CachedShard, NodeState};
use veil_core::ShardId;

/// Inserts/updates shard bytes in cache and updates basic metadata counters.
pub fn cache_put(
    node: &mut NodeState,
    shard_id: ShardId,
    shard_bytes: Vec<u8>,
    now_step: u64,
    ttl_steps: u64,
) {
    node.cache.insert(
        shard_id,
        CachedShard {
            bytes: shard_bytes,
            expiry_step: now_step + ttl_steps,
            last_seen_step: now_step,
        },
    );
    *node.replica_estimate.entry(shard_id).or_insert(0) += 1;
    node.shard_tier
        .entry(shard_id)
        .or_insert(TrustTier::Unknown);
}

/// Inserts shard bytes with WoT-aware cache limits and eviction policy.
#[allow(clippy::too_many_arguments)]
pub fn cache_put_with_policy(
    node: &mut NodeState,
    shard_id: ShardId,
    shard_bytes: Vec<u8>,
    now_step: u64,
    ttl_steps: u64,
    tier: TrustTier,
    max_cache_shards: usize,
    policy: &(impl WotPolicy + ?Sized),
) {
    node.cache.insert(
        shard_id,
        CachedShard {
            bytes: shard_bytes,
            expiry_step: now_step + ttl_steps,
            last_seen_step: now_step,
        },
    );
    if matches!(tier, TrustTier::Trusted | TrustTier::Known) {
        *node.replica_estimate.entry(shard_id).or_insert(0) += 1;
    } else {
        node.replica_estimate.entry(shard_id).or_insert(0);
    }
    node.shard_tier.insert(shard_id, tier);

    evict_expired(node, now_step);

    while evict_over_budget_tiers(node, now_step, policy) {}

    while node.cache.len() > max_cache_shards {
        if !evict_one(node, now_step, policy, None) {
            break;
        }
    }
}

/// Records that a shard was requested, used as an eviction signal.
pub fn note_shard_requested(node: &mut NodeState, shard_id: ShardId) {
    *node.shard_requested.entry(shard_id).or_insert(0) += 1;
}

fn tier_count(node: &NodeState, tier: TrustTier) -> usize {
    node.shard_tier.values().filter(|t| **t == tier).count()
}

fn evict_over_budget_tiers(
    node: &mut NodeState,
    now_step: u64,
    policy: &(impl WotPolicy + ?Sized),
) -> bool {
    let tiers = [
        TrustTier::Blocked,
        TrustTier::Muted,
        TrustTier::Unknown,
        TrustTier::Known,
        TrustTier::Trusted,
    ];
    let mut over_budget: Option<(TrustTier, usize)> = None;
    for tier in tiers {
        let count = tier_count(node, tier);
        let budget = policy.storage_budget(tier);
        if count > budget {
            let over = count - budget;
            match over_budget {
                None => over_budget = Some((tier, over)),
                Some((_, max_over)) if over > max_over => over_budget = Some((tier, over)),
                _ => {}
            }
        }
    }

    if let Some((tier, _)) = over_budget {
        evict_one(node, now_step, policy, Some(tier))
    } else {
        false
    }
}

fn evict_expired(node: &mut NodeState, now_step: u64) {
    let expired: Vec<_> = node
        .cache
        .iter()
        .filter_map(|(sid, cached)| (cached.expiry_step <= now_step).then_some(*sid))
        .collect();
    for sid in expired {
        remove_shard(node, sid);
    }
}

fn evict_one(
    node: &mut NodeState,
    now_step: u64,
    policy: &(impl WotPolicy + ?Sized),
    restrict_tier: Option<TrustTier>,
) -> bool {
    let mut best: Option<(ShardId, f64)> = None;
    for (sid, cached) in &node.cache {
        let tier = *node.shard_tier.get(sid).unwrap_or(&TrustTier::Unknown);
        if let Some(t) = restrict_tier {
            if t != tier {
                continue;
            }
        }
        let meta = ShardMeta {
            tier,
            replica_estimate: *node.replica_estimate.get(sid).unwrap_or(&0),
            age_steps: now_step.saturating_sub(cached.last_seen_step),
            requested_count: *node.shard_requested.get(sid).unwrap_or(&0),
        };
        let priority = policy.eviction_priority(meta);
        match best {
            None => best = Some((*sid, priority)),
            Some((_, best_p)) if priority > best_p => best = Some((*sid, priority)),
            _ => {}
        }
    }

    if let Some((victim, _)) = best {
        remove_shard(node, victim);
        true
    } else {
        false
    }
}

fn remove_shard(node: &mut NodeState, shard_id: ShardId) {
    node.cache.remove(&shard_id);
    node.replica_estimate.remove(&shard_id);
    node.shard_tier.remove(&shard_id);
    node.shard_requested.remove(&shard_id);
}

#[cfg(test)]
mod tests {
    use super::{cache_put, cache_put_with_policy, note_shard_requested};
    use crate::policy::{LocalWotPolicy, TrustTier, WotConfig};
    use crate::state::NodeState;

    #[test]
    fn cache_put_stores_metadata_and_replica_count() {
        let mut node = NodeState::default();
        let shard_id = [0xAA_u8; 32];
        let bytes = vec![1, 2, 3];

        cache_put(&mut node, shard_id, bytes.clone(), 10, 5);

        let stored = node.cache.get(&shard_id).expect("shard should be cached");
        assert_eq!(stored.bytes, bytes);
        assert_eq!(stored.last_seen_step, 10);
        assert_eq!(stored.expiry_step, 15);
        assert_eq!(node.replica_estimate.get(&shard_id), Some(&1));
        assert_eq!(node.shard_tier.get(&shard_id), Some(&TrustTier::Unknown));
    }

    #[test]
    fn cache_put_overwrite_increments_replica_estimate() {
        let mut node = NodeState::default();
        let shard_id = [0xBB_u8; 32];

        cache_put(&mut node, shard_id, vec![1], 1, 10);
        cache_put(&mut node, shard_id, vec![9, 9], 2, 10);

        let stored = node.cache.get(&shard_id).expect("shard should be cached");
        assert_eq!(stored.bytes, vec![9, 9]);
        assert_eq!(stored.last_seen_step, 2);
        assert_eq!(stored.expiry_step, 12);
        assert_eq!(node.replica_estimate.get(&shard_id), Some(&2));
    }

    #[test]
    fn policy_cache_enforces_global_limit() {
        let mut node = NodeState::default();
        let policy = LocalWotPolicy::default();
        for i in 0..3_u8 {
            cache_put_with_policy(
                &mut node,
                [i; 32],
                vec![i],
                10,
                100,
                TrustTier::Unknown,
                2,
                &policy,
            );
        }
        assert!(node.cache.len() <= 2);
    }

    #[test]
    fn policy_cache_enforces_tier_budget() {
        let mut node = NodeState::default();
        let cfg = WotConfig {
            unknown_storage_budget: 1,
            ..WotConfig::default()
        };
        let policy = LocalWotPolicy::new(cfg);

        cache_put_with_policy(
            &mut node,
            [0x01; 32],
            vec![1],
            10,
            100,
            TrustTier::Unknown,
            100,
            &policy,
        );
        cache_put_with_policy(
            &mut node,
            [0x02; 32],
            vec![2],
            11,
            100,
            TrustTier::Unknown,
            100,
            &policy,
        );
        let unknown_count = node
            .shard_tier
            .values()
            .filter(|t| **t == TrustTier::Unknown)
            .count();
        assert_eq!(unknown_count, 1);
    }

    #[test]
    fn policy_cache_rebalances_other_over_budget_tiers() {
        let mut node = NodeState::default();
        let cfg = WotConfig {
            known_storage_budget: 1,
            unknown_storage_budget: 2,
            ..WotConfig::default()
        };
        let policy = LocalWotPolicy::new(cfg);

        cache_put_with_policy(
            &mut node,
            [0x10; 32],
            vec![1],
            10,
            100,
            TrustTier::Known,
            3,
            &policy,
        );
        cache_put_with_policy(
            &mut node,
            [0x11; 32],
            vec![2],
            11,
            100,
            TrustTier::Known,
            3,
            &policy,
        );
        cache_put_with_policy(
            &mut node,
            [0x12; 32],
            vec![3],
            12,
            100,
            TrustTier::Unknown,
            3,
            &policy,
        );

        let known_count = node
            .shard_tier
            .values()
            .filter(|t| **t == TrustTier::Known)
            .count();
        assert_eq!(known_count, 1);
        assert!(node.cache.len() <= 2);
    }

    #[test]
    fn requested_signal_reduces_eviction_pressure() {
        let mut node = NodeState::default();
        let policy = LocalWotPolicy::default();

        let a = [0xA1; 32];
        let b = [0xB2; 32];
        cache_put_with_policy(
            &mut node,
            a,
            vec![1],
            10,
            100,
            TrustTier::Unknown,
            2,
            &policy,
        );
        cache_put_with_policy(
            &mut node,
            b,
            vec![2],
            10,
            100,
            TrustTier::Unknown,
            2,
            &policy,
        );
        for _ in 0..5 {
            note_shard_requested(&mut node, a);
        }
        cache_put_with_policy(
            &mut node,
            [0xC3; 32],
            vec![3],
            20,
            100,
            TrustTier::Unknown,
            2,
            &policy,
        );

        assert!(node.cache.contains_key(&a));
    }

    #[test]
    fn policy_cache_replica_estimate_only_counts_known_and_trusted() {
        let mut node = NodeState::default();
        let policy = LocalWotPolicy::default();

        let unknown = [0x01; 32];
        let known = [0x02; 32];
        let trusted = [0x03; 32];

        cache_put_with_policy(
            &mut node,
            unknown,
            vec![1],
            10,
            100,
            TrustTier::Unknown,
            10,
            &policy,
        );
        cache_put_with_policy(
            &mut node,
            known,
            vec![2],
            10,
            100,
            TrustTier::Known,
            10,
            &policy,
        );
        cache_put_with_policy(
            &mut node,
            trusted,
            vec![3],
            10,
            100,
            TrustTier::Trusted,
            10,
            &policy,
        );

        assert_eq!(node.replica_estimate.get(&unknown), Some(&0));
        assert_eq!(node.replica_estimate.get(&known), Some(&1));
        assert_eq!(node.replica_estimate.get(&trusted), Some(&1));
    }
}
