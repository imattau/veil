use std::collections::{HashMap, HashSet};
use serde::{Deserialize, Serialize};
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
    /// Pending ACK entries for timeout escalation.
    pub pending_acks: HashMap<ObjectRoot, PendingAck>,
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
        assert!(state.pending_acks.is_empty());
    }
}
