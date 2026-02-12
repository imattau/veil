//! VEIL node-layer primitives and runtime helpers.
//!
//! This crate wires together shard reception, forwarding, reconstruction,
//! policy-aware caching, and ACK handling on top of pluggable transports.
//!
//! Typical integration loop:
//! 1. Publisher side: enqueue app payloads into [`batch::FeedBatcher`], then call
//!    [`publish::publish_service_tick_multi_lane`] each step.
//! 2. Network side: move outbound transport bytes between peers/adapters.
//! 3. Subscriber/runtime side: call [`runtime::pump_multi_lane_tick_with_config`]
//!    to ingest shards, forward, reconstruct, and auto-emit ACK objects.
//! 4. Publisher runtime: keep ticking so inbound ACK objects clear pending
//!    retry state.

pub mod ack;
pub mod batch;
pub mod bloom;
pub mod cache;
pub mod config;
pub mod forwarding;
pub mod persistence;
pub mod policy;
pub mod publish;
pub mod receive;
pub mod runtime;
pub mod service;
pub mod state;
pub mod subscriptions;
