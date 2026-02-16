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
- [x] **Bug Fixes & Maintenance**:
    - [x] Fix `NodeState.copyWith` silently clearing `lastError`.
    - [x] Fix infinite media fetch retry loop with no backoff in `SocialController`.
    - [x] Fix optimistic profile hardcoded `seq: 999999` blocking real updates.
    - [x] Fix `NodeService._addFeedEvent` double `notifyListeners`.
    - [x] Fix `_formatTime` showing "0m" for recent posts.
    - [x] Fix silent publish failures in `ComposerView` and `ProfileEditView`.
    - [x] Fix existing avatar not loading in `ProfileEditView` (UX-1).
    - [x] Fix hardcoded white background in `SemanticFeedCard` (UX-2).
    - [x] Implement navigation for hashtags/mentions/links in `RichTextView` (UX-3).
    - [x] Add image lightbox for media expand in `VeilPostCard` (UX-4).
    - [x] Upgrade `FeedShimmer` to use gradient sweep (UX-5).
    - [x] Add character count and limits to `ComposerView` (UX-6).
    - [x] Implement vote deduplication in `PollWidget` (UX-7).
    - [x] Add dismiss/action to `_BackupReminder` (UX-8).
- [x] **List Objects & Preferences**:
    - [x] Implement `publishList()` and `publishAppPreferences()` in `NodeService`.
    - [x] Add "latest-wins" cache for lists and preferences in `NodeService`.
    - [x] Implement `PreferencesController` for app-wide settings sync.
    - [x] Add bookmark action to `VeilPostCard`.
    - [x] Create Bookmarks view and Settings screen.
    - [x] Document `/list` and `/app_preferences` in `node_rpc.md`.
- [x] Implement `LinkPreviewCard`.
- [x] Add `MediaGrid` support.
- [x] Enhance `ComposerView` with social parsing.
- [x] **Networking Stability**:
    - [x] Sync persisted contacts to `ProtocolEngine` on startup (#21).
    - [x] Persist `encrypt_key` instead of random generation (#22).
    - [x] Log `pump_inbound` errors instead of silently swallowing (#23).
    - [x] Fix non-routable self-contact in discovery gossip (#24).
    - [x] Prioritize HTTP gossip for initial peer bootstrap (#25).
    - [x] Add WebSocket adapter auto-reconnection (#26).
- [x] **Media Fixes**:
    - [x] Fix `reconstruct_payload` to CBOR-unwrap batch and match individual item hashes.
    - [x] Update `publish_object` to return the pipeline-consistent root hash.
    - [x] Verify `emit_payload` root consistency for media.
    - [x] Add round-trip integration test for media upload/fetch.
- [x] **M9 - Social UX Refinement**:
    - [x] Add like/unlike toggle to `_PostFooter` heart button (#13).
    - [x] Add repost toggle with confirmation dialog (#14).
    - [x] Remove duplicate `ReactionTray` or merge with footer (#15).
    - [x] Make `ReactionTray` chips tappable (#16).
    - [x] Fix comment button on detail view (#17).
    - [x] Increase tap targets to 48x48dp minimum (#18).
    - [x] Add optimistic UI for like toggle (#19).
    - [x] Cache `ZapController` instance instead of creating per tap (#20).
- [x] **M10 - Stability & Hardening**:
    - [x] Fix WS stream leak and dispose cleanup (#21, #22).
    - [x] Replace global busy flag with per-operation tracking (#24).
    - [x] Secure and harden `ZapController` external calls (#25, #37).
    - [x] Parallelize `refresh()` and fix poller stacking (#27, #28).
    - [x] Add disposal guards to `SocialController` and others (#30, #31).
    - [x] Implement WS backoff and offline detection (#26, #33).
    - [x] Fix unbounded `_feedEvents` growth (#32).
    - [x] Debounce `MessagingController` notifications (#34).
    - [x] Fix race conditions in `ListController` and `PreferencesController` (#35, #36).
    - [x] Clean up `NetworkPulse` and `ServiceControls` (#29, #38, #39).
    - [x] Address P2 minor improvements (#40, #43, #44, #45).
- [ ] **M11 - UI/Logic Monolith Decomposition + P0 Regression Fixes**:
    - [x] **Monolith inventory**: 47 Dart files / ~9,900 lines, with 6 critical monoliths identified.
    - [x] **Task 1 (P0)**: Split `logic/node_service.dart` (`1679` lines, `54` methods) into focused modules.
    - [x] **Task 2**: Decompose `logic/social_controller.dart` (`545` lines, `29` methods) into focused modules.
    - [x] **Task 3**: Extract `screens/connections_view.dart` widgets (`876` lines, `9` classes).
    - [x] **Task 4**: Extract `screens/profile_view.dart` widgets (`705` lines, `8` classes + function).
    - [x] **Task 5**: Extract `components/veil_post_card.dart` widgets (`586` lines, `4` classes + enum).
    - [x] **Task 6**: Extract `screens/composer_view.dart` widgets (`486` lines, `5` classes).
    - [x] **Task 7 (P0)**:
        - [x] Fix WebSocket subscription leak on reconnect.
        - [x] Fix async cleanup path in `dispose()`.
        - [x] Replace/verify global busy-flag behavior with per-operation tracking.
    - [x] **Task 8**: Final import + barrel/export integration pass.
    - [x] **Dependency graph enforcement**:
        - [x] Task 1 -> Task 2
        - [x] Task 1 -> Task 7
        - [x] Tasks 1/3/4/5/6 parallelizable
        - [x] Task 8 last
- [x] **M12 - Social UI/UX Cohesion + Preference Wiring**:
    - [x] **Task 1 (P0) - Theme + preference wiring**:
        - [x] Lift `PreferencesController` to app root and drive `MaterialApp` from persisted prefs.
        - [x] Wire settings theme choices (`dark`/`light`/`amoled`) to actual runtime theme selection.
        - [x] Add and tune `VeilTheme` variants for `light` and `amoled` while preserving current dark style.
    - [x] **Task 2 (P0) - Shell layout consistency**:
        - [x] Normalize top/bottom shell insets across Home, Explore, Inbox, and Profile tabs.
        - [x] Ensure list/content surfaces do not render under glass app bar/navigation chrome.
        - [x] Verify pull-to-refresh and FAB spacing behavior on all tabs.
    - [x] **Task 3 (P1) - Navigation + action clarity**:
        - [x] Normalize "tap active tab to scroll-to-top" behavior across all tabs (or remove where unsupported).
        - [x] Replace icon-only contextual FAB affordances with explicit per-tab actions (label or equivalent clarity).
        - [x] Align channel membership CTA copy/state in Explore (`Join`/`Leave`) and feedback toasts.
    - [x] **Task 4 (P1) - Composer + onboarding polish**:
        - [x] Apply saved `default_channel` preference when opening `ComposerView`.
        - [x] Disable publish action when no text/media is present and provide clear disabled affordance.
        - [x] Improve first-run empty-state onboarding path from feed/explore into follow/join flows.
    - [x] **Task 5 (P2) - Accessibility + visual QA**:
        - [x] Run contrast, focus, and minimum tap target audit across core social screens.
        - [x] Standardize spacing/radius/elevation token usage across cards, dialogs, and sheets.
        - [x] Add widget/golden smoke coverage for Home, Explore, Inbox, Profile, and Composer shells.
    - [x] **Dependency graph enforcement**:
        - [x] Task 1 -> Task 4
        - [x] Task 2 -> Task 3
    - [x] Tasks 1/2 parallelizable
    - [x] Task 5 after Tasks 1-4
- [x] **M13 - Threaded Conversations in Comments and Messaging**:
    - [x] **Task 1 (P0) - Post comment threading**:
        - [x] Render nested comment threads by traversing `reply_to_root` relationships in `PostDetailView`.
        - [x] Add explicit per-comment reply actions and visual nesting/indentation.
        - [x] Wire reply composer to a selected comment target with clear/cancel affordance.
    - [x] **Task 2 (P0) - DM/group message threading**:
        - [x] Add reply-target selection on message long-press in `ChatDetailView`.
        - [x] Send message replies with `replyToRoot` for both DM and group publish paths.
        - [x] Render inline reply context inside message bubbles when referenced messages are present.
    - [x] **Task 3 (P1) - Coverage and validation**:
        - [x] Extend widget coverage for nested post-comment rendering and targeted replies.
        - [x] Add widget coverage verifying DM reply sends include `replyToRoot`.
        - [x] Run analyzer and targeted test suite after implementation.
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
