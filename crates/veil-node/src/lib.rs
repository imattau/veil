//! VEIL node-layer primitives and runtime helpers.
//!
//! This crate wires together shard reception, forwarding, reconstruction,
//! policy-aware caching, and ACK handling on top of pluggable transports.

pub mod ack;
pub mod batch;
pub mod cache;
pub mod config;
pub mod forwarding;
pub mod policy;
pub mod publish;
pub mod receive;
pub mod runtime;
pub mod state;
