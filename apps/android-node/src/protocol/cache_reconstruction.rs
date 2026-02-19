use veil_core::ObjectRoot;
use veil_fec::profile::ErasureCodingMode;
use veil_fec::sharder::reconstruct_object_padded_with_mode;
use veil_node::state::NodeState;

use super::erasure_mode_from_shards;
use super::payload_reconstruction::shards_for_root;

pub(super) fn cached_shard_bytes(state: &NodeState, shard_id: [u8; 32]) -> Option<Vec<u8>> {
    state
        .cache
        .get(&shard_id)
        .map(|cached| cached.bytes.clone())
}

pub(super) fn reconstruct_cached_object(
    state: &NodeState,
    root: ObjectRoot,
    fallback_mode: ErasureCodingMode,
) -> Option<Vec<u8>> {
    let shards = shards_for_root(state, root);
    if shards.is_empty() {
        return None;
    }
    let mode = erasure_mode_from_shards(&shards, fallback_mode);
    reconstruct_object_padded_with_mode(&shards, root, mode).ok()
}
