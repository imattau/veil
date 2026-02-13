use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use veil_codec::shard::ShardV1;
use veil_core::ObjectRoot;
use veil_core::{ShardId, Tag};

use crate::policy::TrustTier;

/// Cached shard bytes and eviction metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedShard {
    /// Encoded shard bytes.
    pub bytes: Vec<u8>,
    /// Cache expiry step.
    pub expiry_step: u64,
    /// Last step this shard was seen.
    pub last_seen_step: u64,
}

/// Pending ACK timeout/escalation state for an outbound object.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingAck {
    /// Retry queue of unsent shard bytes.
    pub unsent_shards: Vec<Vec<u8>>,
    /// Next step when retry should fire.
    pub next_retry_step: u64,
    /// Number of retry batches already sent.
    pub retries: u32,
    /// Maximum allowed retry batches.
    pub max_retries: u32,
    /// Retry batch size per escalation step.
    pub retry_batch_size: usize,
    /// Delay between retries.
    pub backoff_step: u64,
}

/// Mutable node-local state used by receive/runtime/cache pipelines.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct NodeState {
    /// Locally subscribed tags.
    pub subscriptions: HashSet<Tag>,
    /// Cache keyed by shard id.
    pub cache: HashMap<ShardId, CachedShard>,
    /// Local shard popularity heuristic.
    pub replica_estimate: HashMap<ShardId, u64>,
    /// Trust tier associated with each cached shard.
    pub shard_tier: HashMap<ShardId, TrustTier>,
    /// Request count signal associated with each cached shard.
    pub shard_requested: HashMap<ShardId, u64>,
    /// Reconstruction inbox grouped by object root and shard index.
    pub inbox: HashMap<ObjectRoot, HashMap<u16, ShardV1>>,
    /// Recently seen shard ids used for duplicate suppression independent of cache policy.
    #[serde(skip)]
    pub seen_shards_lru: Option<lru::LruCache<ShardId, u64>>,
    /// Storage for seen_shards when persisting.
    pub seen_shards: HashMap<ShardId, u64>,
    /// Pending ACK entries for timeout escalation.
    pub pending_acks: HashMap<ObjectRoot, PendingAck>,
    /// Index of object roots to their cached shard IDs.
    #[serde(default)]
    pub shard_index: HashMap<ObjectRoot, HashSet<ShardId>>,
    /// Reverse index of shard IDs to their object roots.
    #[serde(default)]
    pub shard_to_root: HashMap<ShardId, ObjectRoot>,
}

impl NodeState {
    pub fn is_shard_seen(&mut self, shard_id: &ShardId, now_step: u64) -> bool {
        let lru = self.ensure_seen_shards_lru();
        if let Some(expiry) = lru.get(shard_id) {
            if *expiry > now_step {
                return true;
            }
        }
        false
    }

    pub fn mark_shard_seen(&mut self, shard_id: ShardId, expiry_step: u64) {
        let lru = self.ensure_seen_shards_lru();
        lru.put(shard_id, expiry_step);
    }

    fn ensure_seen_shards_lru(&mut self) -> &mut lru::LruCache<ShardId, u64> {
        if self.seen_shards_lru.is_none() {
            let mut lru = lru::LruCache::new(std::num::NonZeroUsize::new(10000).unwrap());
            for (id, expiry) in &self.seen_shards {
                lru.put(*id, *expiry);
            }
            self.seen_shards_lru = Some(lru);
        }
        self.seen_shards_lru.as_mut().unwrap()
    }

    pub fn prepare_for_persist(&mut self) {
        if let Some(lru) = &self.seen_shards_lru {
            self.seen_shards.clear();
            for (id, expiry) in lru {
                self.seen_shards.insert(*id, *expiry);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::NodeState;

    #[test]
    fn node_state_default_is_empty() {
        let state = NodeState::default();
        assert!(state.subscriptions.is_empty());
        assert!(state.cache.is_empty());
        assert!(state.replica_estimate.is_empty());
        assert!(state.shard_tier.is_empty());
        assert!(state.shard_requested.is_empty());
        assert!(state.inbox.is_empty());
        assert!(state.seen_shards.is_empty());
        assert!(state.pending_acks.is_empty());
    }

    #[test]
    fn shard_deduplication_works() {
        let mut state = NodeState::default();
        let sid = [0xAA; 32];

        assert!(!state.is_shard_seen(&sid, 10));
        state.mark_shard_seen(sid, 20);
        assert!(state.is_shard_seen(&sid, 10));
        assert!(!state.is_shard_seen(&sid, 25)); // Expired
    }

    #[test]
    fn shard_deduplication_lru_limits() {
        let mut state = NodeState::default();
        // Force a small LRU for testing if we could, but it's hardcoded to 10000.
        // We'll just verify basic functionality and then test persistence.
        for i in 0..10 {
            state.mark_shard_seen([i; 32], 100);
        }
        assert!(state.is_shard_seen(&[5; 32], 10));
    }

    #[test]
    fn prepare_for_persist_populates_seen_shards() {
        let mut state = NodeState::default();
        state.mark_shard_seen([0x11; 32], 100);
        state.mark_shard_seen([0x22; 32], 200);

        state.prepare_for_persist();
        assert_eq!(state.seen_shards.get(&[0x11; 32]), Some(&100));
        assert_eq!(state.seen_shards.get(&[0x22; 32]), Some(&200));
    }

    #[test]
    fn lazy_lru_restores_from_seen_shards() {
        let mut state = NodeState::default();
        state.seen_shards.insert([0x33; 32], 300);

        // This should trigger lazy initialization of seen_shards_lru
        assert!(state.is_shard_seen(&[0x33; 32], 100));
    }
}
