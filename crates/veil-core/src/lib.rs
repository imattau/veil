//! Core VEIL primitives shared across crates.
//!
//! Includes fixed-size types, hash helpers, tag derivation, and base errors.

pub mod error;
pub mod hash;
pub mod tags;
pub mod types;

pub use types::{Epoch, Namespace, ObjectRoot, ShardId, Tag};
