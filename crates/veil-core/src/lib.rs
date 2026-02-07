//! Core VEIL primitives shared across crates.
//!
//! Includes fixed-size types, hash helpers, tag derivation, and base errors.

pub mod error;
pub mod hash;
pub mod tags;
pub mod types;

pub use types::{
    Epoch, Namespace, ObjectRoot, ShardId, Tag, NAMESPACE_APP_BUNDLE, NAMESPACE_PRIVATE_VAULT,
    NAMESPACE_PUBLIC_FEED, NAMESPACE_RELAY, NAMESPACE_RESERVED_MAX, NAMESPACE_SYSTEM,
    NAMESPACE_WOT,
};
