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

/// Reserved namespace values (protocol/system range).
pub const NAMESPACE_SYSTEM: Namespace = Namespace(0);
pub const NAMESPACE_PUBLIC_FEED: Namespace = Namespace(1);
pub const NAMESPACE_PRIVATE_VAULT: Namespace = Namespace(2);
pub const NAMESPACE_WOT: Namespace = Namespace(3);
pub const NAMESPACE_RELAY: Namespace = Namespace(4);
pub const NAMESPACE_APP_BUNDLE: Namespace = Namespace(5);
pub const NAMESPACE_RESERVED_MAX: Namespace = Namespace(31);

/// Epoch/window identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Epoch(pub u32);
