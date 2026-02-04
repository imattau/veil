//! Transport abstractions for VEIL.
//!
//! The node/runtime depends on the byte-oriented `adapter::TransportAdapter`.
//! `lane::TransportLane` is legacy and kept for compatibility only.

pub mod adapter;
pub mod lane;
