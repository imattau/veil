use crate::state::NodeState;
use veil_core::ShardId;

/// Returns whether a shard should be forwarded:
/// not already cached and tag is subscribed.
pub fn should_forward(node: &NodeState, shard_id: ShardId, tag: &[u8; 32]) -> bool {
    !node.cache.contains_key(&shard_id) && node.subscriptions.contains(tag)
}

#[cfg(test)]
mod tests {
    use super::should_forward;
    use crate::{cache::cache_put, state::NodeState};

    #[test]
    fn forwards_when_new_and_subscribed() {
        let mut node = NodeState::default();
        let tag = [0x10_u8; 32];
        let shard_id = [0x22_u8; 32];
        node.subscriptions.insert(tag);

        assert!(should_forward(&node, shard_id, &tag));
    }

    #[test]
    fn does_not_forward_when_not_subscribed() {
        let node = NodeState::default();
        let tag = [0x10_u8; 32];
        let shard_id = [0x22_u8; 32];

        assert!(!should_forward(&node, shard_id, &tag));
    }

    #[test]
    fn does_not_forward_when_already_cached() {
        let mut node = NodeState::default();
        let tag = [0x10_u8; 32];
        let shard_id = [0x22_u8; 32];
        node.subscriptions.insert(tag);
        cache_put(&mut node, shard_id, vec![1, 2], 0, 10);

        assert!(!should_forward(&node, shard_id, &tag));
    }
}
