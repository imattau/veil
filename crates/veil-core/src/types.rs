use serde::{Deserialize, Serialize};

/// 32-byte opaque subscription identifier.
pub type Tag = [u8; 32];
/// 32-byte hash grouping shards for one encoded object.
pub type ObjectRoot = [u8; 32];
/// 32-byte hash identifier for dedupe (`hash(shard_bytes)`).
pub type ShardId = [u8; 32];

/// Logical namespace identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Namespace(pub u16);

/// Epoch/window identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Epoch(pub u32);
