# VEIL Master Implementation Plan

This document consolidates all feature-specific plans and roadmaps for the VEIL project.

## 1. Core Protocol & Library (`crates/*`)

### M1 - Spec Lock + Vectors
- [x] Freeze v0.1 fields/flags and tag derivations (`feed_tag`, `rv_tag`).
- [x] Publish deterministic vectors for tags, object headers, and shard headers.
- [x] Add Rust `veil-core` helpers for deterministic channel-scoped namespace/tag derivation.
- [x] Add test vectors for channel namespace derivation (normalization + determinism).

### M2 - Object Pipeline
- [x] Implement batching (`TARGET_BATCH_SIZE`) and fast interactive flush.
- [x] Implement object build: encrypt, optional sign, padding to bucket-friendly sizes.
- [x] Define endorsement object schema (non-normative).
- [x] Verify and ingest endorsements from runtime objects.
- [x] Add duplicate suppression + staleness pruning.

### M3 - Sharding + Reconstruction
- [x] Implement profile/bucket selection and systematic Reed-Solomon split.
- [x] Generate `shard_id = H(shard_bytes)` and enforce dedupe semantics.
- [x] Property tests for reconstruction from any `k` unique shard indices.
- [x] Support systematic mode for public posts through namespace-aware erasure mode selection.

### M4 - Node Forwarding + Cache
- [x] Enforce subscription-based forwarding by `tag`.
- [x] Implement TTL cache and rarity-biased eviction using local replica heuristics.
- [x] Add WoT-aware prioritization hooks (tiered quotas/caps) behind default-compatible policy.
- [x] **Durable Persistence**: Snapshotted and restored `NodeState` across restarts (subscriptions, cache, replica estimates, etc.).
- [x] **WoT Core**: Deterministic `score_publisher`, explainability payload, and boundary tests.
- [x] **WoT Graph**: JSON export/import for local WoT state.
- [x] **Cache Policy**: Enforce per-tier caps and tune weights (rarity/trust/age/request).

### M5 - Multi-lane Delivery + ACK
- [x] Lane A sends `k+2` shards to two peers; Lane B sends fallback shards.
- [x] Add escalation on ACK timeout with backoff and bounded retries.
- [x] Implement transport-driven ingest loop reading from adapters.
- [x] **Runner Orchestration**: Built-in runtime loop with tick/sleep/error handling.

### M6 - Hardening + Release Candidate
- [x] Add fuzzing for codec/parser boundaries.
- [x] Add end-to-end example (`object -> shards -> forward -> reconstruct -> ACK`).
- [x] Ensure `cargo fmt`, `clippy`, and `cargo test --workspace` are all green.

### M7 - Networking Stability & Optimization
- [x] **Shard Indexing**: Index cached shards by `object_root` to avoid full cache scans during reconstruction.
- [x] **Adaptive Ticking**: Implement adaptive sleep in `QueueWorker` (Android) to handle bursty traffic efficiently.
- [x] **QUIC SNI Flex**: Support per-peer server names in QUIC adapter to allow connecting to multiple distinct VPS nodes.
- [x] **Error Clarity**: Add granular error codes to `LaneDetail` for better remote diagnostics of transport failures.

### M8 - Library & UX Standardisation
- [x] **Standard CLI**: Migrate `vps-node` and simulation tools to `clap` for robust argument parsing and help generation.
- [x] **Declarative Config**: Replace manual `setting_` helpers in VPS with a `serde`-based `Config` struct and `config-rs`.
- [x] **Human-Readable Units**: Support `humantime` durations (e.g., \"5s\") and `byte-unit` sizes (e.g., \"128KiB\") in configuration.
- [x] **Unified Lazy Init**: Replace `Arc<Mutex<Option<T>>>` with `OnceLock` or `once_cell` for shared resources.
- [ ] **Concurrent Collections**: Evaluate `DashMap` for high-contention shard/peer tables to reduce global lock contention.
- [ ] **Structured Logs**: Finish migrating all crates from `eprintln!` to `tracing` macros.

---

## 2. Transport Adapters (`crates/veil-transport-*`)

### WebSocket Adapter
- [x] Concrete implementation with `send`/`recv`.
- [x] Reconnect with exponential backoff.
- [x] Bounded outbound/inbound buffering.
- [x] Metrics hooks (queue/send/recv/reconnect).

### Tor SOCKS5 Adapter
- [x] Queued outbound sends through SOCKS5 proxy.
- [x] Connect/send timeouts and background worker.
- [x] Metrics hooks (attempts/success/errors).

### QUIC Fast-Lane Adapter
- [x] Real fast-lane transport with low-latency connectivity.
- [x] Uni-stream send/recv paths.
- [x] Self-signed identity generator and trust-store based client config.
- [x] Metrics hooks (attempts/success/errors/recv drops).

### BLE Adapter
- [x] Integrated into transport model via `btleplug`.

### Unified Health Surface
- [x] Uniform `TransportHealthSnapshot` trait across all adapters.
- [x] Snapshot APIs exposed on each adapter for polling health.

---

## 3. Client & SDKs (`packages/*`)

### JS SDK (`veil-sdk-js`)
- [x] Rust `veil-wasm` bindings for browser/Node.
- [x] JS-safe primitives (`deriveFeedTag`, `deriveRvTag`, etc.).
- [x] Backend selection (`auto` / `wasm` / `pure-js`).
- [x] WebSocket lane with reconnect/backoff.
- [x] WebRTC lane adapter adapter.
- [x] SDK `VeilClient` runtime loop (ingest, forward, subscribe).
- [x] Persistent cache (IndexedDB for browser, AsyncStorage/MMKV for React Native).
- [x] Key management helpers (WebCrypto).
- [x] Expose WoT score/explanation and trust import/export.

### Dart/Flutter SDK (`veil-sdk-dart`)
- [x] Rust core wrap via Flutter Rust Bridge.
- [x] Support for multi-lane transports and cache stores.

---

## 4. Applications (`apps/*`)

### Android Node (`apps/android-node`)
- [x] Local RPC schema (HTTP+WS) and embedded node binary.
- [x] Foreground service wrapper with lifecycle management.
- [x] Authenticated localhost RPC endpoint.
- [x] Identity management (creation/persistence/rotation).
- [x] Shard cache persistence.
- [x] **Social Engine**: Sequence-based feed sorting, silent onboarding.
- [x] **Messaging**: Payload handling, decrypted payload caching.
- [x] **UI Components**: `VeilPostCard`, `ReactionTray`, `PollWidget`, `LiveStatusBanner`.
- [x] **Visuals**: Tabbed navigation, Glassmorphism, Network Pulse.
- [x] **Rich Content**: Clickable hashtags/mentions/links, Nested Boosts.
- [ ] Implement `LinkPreviewCard`.
- [ ] Add `MediaGrid` support.
- [ ] Enhance `ComposerView` with social parsing.
- [ ] Implement unit tests for rich content.

### VPS Node (`apps/veil-vps-node`)
- [x] Production edge forwarder profile.
- [x] SQLite settings and Nostr identity bridge.
- [x] Admin web dashboard with peer discovery and settings management.
- [x] **Efficiency Rollout**: Default-on bloom exchange and probabilistic forwarding.

---

## 5. Deployment & Rollout

### Protocol Efficiency Rollout
- [x] VPS: enable `probabilistic_forwarding` and `bloom_exchange`.
- [x] Android: enable conservative defaults.
- [ ] **Stage 1**: Deploy VPS with default-on efficiency features.
- [ ] **Stage 2**: Deploy Android with matching features.
- [ ] **Stage 3**: Observe traffic/latency and tune parameters (min prob, replica divisor, bloom interval).

### Release Gates (0.1.0-rc1)
- [x] Functional: tag derivation, schema compliance, and ACK behavior.
- [x] Resilience: packet loss tolerance and cache churn behavior in `veil-sim`.
- [x] Performance: throughput, p95 latency, and cache hit rate baselines recorded.
- [x] Transport-agnostic: validated over at least two adapter implementations.
- [x] Policy-locality: WoT settings only affect prioritization, not validity.
