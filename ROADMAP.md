# VEIL Project Roadmap (v0.1)

## Objective
Deliver a practical-mode VEIL implementation in this Rust workspace that can:
1) build/encrypt objects, 2) erasure-code into shards, 3) forward across multi-lane transports, 4) reconstruct/decrypt at subscribers, and 5) maintain high shard coverage with rarity-biased caching.

## Scope and Defaults
- Profiles: `SMALL(k=6,n=10)` and `LARGE(k=10,n=16)`
- Buckets: `16 KiB`, `32 KiB`, `64 KiB`
- Limits: `MAX_OBJECT_SIZE=256 KiB`, `TARGET_BATCH_SIZE=96 KiB`
- Epoch mode: `EPOCH_SECONDS=86400` (24h)
- Cache TTL: `90 minutes` (or equivalent simulation steps)

## Workstreams by Crate
- `veil-core`: tags, hashes, fixed types (`Tag`, `ObjectRoot`, `ShardId`), and error model
- `veil-codec`: canonical `ObjectV1` + `ShardV1` encode/decode and validation
- `veil-crypto`: AEAD encrypt/decrypt and optional signatures
- `veil-fec`: profile selection, RS sharding/reconstruction, shard sizing
- `veil-node`: subscription filter, dedupe/cache, inbox/reconstruction, ACK flow
- `veil-transport`: lane abstraction and multi-lane send policy
- `veil-sim`: packet loss/latency scenarios and cache-pressure behavior
- `policy` (in `veil-node` initially): local WoT-based forwarding/cache/UI prioritization hooks

## Feasibility Assessment (Transport-Agnostic Notes)
- **Feasible with low risk**: current architecture already keeps protocol semantics in `veil-core`/`veil-codec` and policy/runtime in `veil-node`.
- **Low-impact path**: evolve `veil-transport` into a byte-blob adapter contract without changing shard/object wire formats.
- **No protocol changes required**: lane identity remains local policy and is not encoded in shard headers.
- **Incremental rollout**: implement adapter + runtime loop behind new APIs, then migrate existing paths without breaking tests/examples.

## Feasibility Assessment (WoT Policy Notes)
- **Feasible with low risk**: WoT is a local prioritization layer; it does not require protocol, shard header, or transport changes.
- **Minimum-impact path**: add policy hooks in `veil-node` (`classify`, `quota`, `budget`, `eviction_priority`) with safe defaults that preserve current behavior.
- **No global trust requirement**: v1 uses local follow/mute/block plus bounded endorsements (depth <= 2, thresholded).
- **Pipeline invariance**: WoT influences ordering and quotas only; object validity, reconstruction, and delivery logic remain unchanged.

## Minimal-Impact Integration Plan (Addendum)
### T1 - Transport Adapter Contract (`veil-transport`)
- Introduce an adapter trait focused on opaque bytes:
  - `send(peer, bytes)`
  - `recv() -> (peer, bytes)`
  - opaque peer handle for replies
  - optional `max_payload_hint()`
- Keep existing lane interfaces as compatibility wrappers during migration.
- Exit criteria: mock adapter tests prove lossy/unordered delivery is tolerated.

### T2 - Node Runtime Loop (`veil-node`)
- Add a transport-driven ingest loop that reads from one or more adapters and routes bytes into shard processing.
- Keep current shard pipeline unchanged: dedupe/cache -> subscription gate -> forward -> reconstruct -> decrypt -> app callback.
- Accept inbound-only or outbound-only adapters.
- Exit criteria: node receives from adapter and delivers decrypted payload in integration tests.

### T3 - Multi-Lane Local Policy (`veil-node` + `veil-transport`)
- Implement lane selection as local policy only (fast lane + fallback lane), with no header/schema impact.
- Use coarse transport capabilities (payload hint) to choose shard/bucket send strategy.
- Exit criteria: sim run shows delivery success under partial lane failure.

### T4 - Cache Pressure + Rarity Bias (`veil-node`)
- Finalize eviction behavior: drop expired first, then evict most common by local observations.
- Preserve local signals for future WoT/payment weighting without affecting validity rules.
- Exit criteria: under constrained cache, rare shards have longer residency than common shards.

### T5 - WoT Policy Hooks (`veil-node`)
- Add local trust tiers: `Trusted`, `Known`, `Unknown`, `Muted`, `Blocked`.
- Add policy interface for:
  - `classify_publisher(pubkey) -> tier`
  - `forwarding_quota(tier) -> fraction`
  - `storage_budget(tier) -> max_shards`
  - `eviction_priority(meta) -> score`
- Default v1 policy:
  - explicit follows -> `Trusted`, blocks -> `Blocked`, mutes -> `Muted`
  - `Known` via >=2 trusted endorsers, max depth 2, strong decay
  - forwarding budget 70/25/5 for Trusted/Known/Unknown (Muted ~0, Blocked 0)
- Exit criteria: policy toggles change forwarding/cache priorities without changing validation results.

## Milestones

### M1 - Spec Lock + Vectors
- Freeze v0.1 fields/flags and tag derivations (`feed_tag`, `rv_tag`)
- Publish deterministic vectors for tags, object headers, and shard headers
- Exit criteria: vectors pass in CI across all relevant crates

### M2 - Object Pipeline
- Implement batching (`TARGET_BATCH_SIZE`) and fast interactive flush
- Implement object build: encrypt, optional sign, padding to bucket-friendly sizes
- Exit criteria: object round-trip tests plus signature/AEAD negative tests

### M3 - Sharding + Reconstruction
- Implement profile/bucket selection and systematic Reed-Solomon split
- Generate `shard_id = H(shard_bytes)` and enforce dedupe semantics
- Exit criteria: property tests reconstruct from any `k` unique shard indices

### M4 - Node Forwarding + Cache
- Enforce subscription-based forwarding by `tag`
- Implement TTL cache and rarity-biased eviction using local replica heuristics
- Add WoT-aware prioritization hooks (tiered quotas/caps) behind default-compatible policy
- Exit criteria: under pressure, rare shards survive longer than common shards

### M5 - Multi-lane Delivery + ACK
- Lane A sends `k+2` shards to two peers; Lane B sends fallback shards
- Add escalation on ACK timeout with backoff and bounded retries
- Land transport-adapter runtime loop for inbound/outbound byte payloads
- Exit criteria: delivery succeeds in degraded-lane simulation scenarios

### M6 - Hardening + Release Candidate
- Add fuzzing for codec/parser boundaries
- Add end-to-end example (`object -> shards -> forward -> reconstruct -> ACK`)
- Exit criteria: `cargo fmt`, `clippy -D warnings`, and `cargo test --workspace` all green

## Android Node UI Roadmap (App Over Local Node)

### Phase 0 - Contracts + Baseline
- [x] Define local RPC schema (requests/events) with versioning.
- [x] Choose IPC transport (localhost HTTP+WS first).
- [x] Do not use the SDK in android-node; UI must talk directly to the node API.
- [x] Define identity/storage boundaries (node owns keys, cache, queue).
- [x] Define observability contract (lane health, queue depth, shard stats, errors).
- [x] Exit criteria: RPC spec doc + stub client in app.

### Phase 1 - Node as Foreground Service
- [x] Android foreground service wrapper for the node.
- [x] Start/stop lifecycle and persistent notification.
- [x] Authenticated localhost RPC endpoint.
- [x] Exit criteria: UI can connect and read node status.

### Phase 2 - Transport + Identity
- [x] Node manages QUIC/WS/Tor lanes and exposes health.
- [x] Node-managed identity creation/persistence/rotation.
- [x] UI shows identity and lane status.
- [x] Exit criteria: UI can display live lane health + identity.

### Phase 3 - Messaging + Publish Queue
- [x] Node owns publish queue with offline buffering and retries.
- [x] UI submits payloads and receives status updates.
- [x] Exit criteria: UI send works offline and drains on reconnect.

### Phase 4 - Shard Cache + Reconstruction
- [x] Node stores shards, reconstructs objects, validates signatures.
- [x] UI receives decrypted semantic messages via event stream.
- [x] Exit criteria: end-to-end message flow over multiple lanes.

### Phase 5 - WoT + Local Policy
- [x] Node computes trust tiers and enforces routing/cache policy.
- [x] UI displays trust summaries and policy controls.
- [x] Exit criteria: policy changes affect routing without protocol changes.

### Phase 6 - Reliability + Diagnostics
- [x] Crash recovery and data migrations.
- [x] Diagnostics UI for lane health, queue, storage, errors.
- [x] Exit criteria: reproducible E2E tests + observability dashboard.

### Phase 7 - Discovery + Endorsements
- [x] Endorsement payloads update local WoT policy automatically.
- [x] Policy explanation + update endpoints for diagnostics.
- [x] Persistent shard cache across restarts.
- [x] Inbound QUIC listener for true P2P.
- [x] Object/shard retrieval endpoints for clients.
- [x] Auto discovery/contact exchange (LAN broadcast + gossip + DHT lookup).

## Release Gates (0.1.0-rc1)
- Functional: tag derivation, schema compliance, and ACK behavior
- Resilience: packet loss tolerance and cache churn behavior in `veil-sim`
- Performance: throughput, p95 end-to-end latency, and cache hit rate baselines
- Transport-agnostic: same shard/object pipeline passes over at least two adapter implementations (e.g., in-memory mock + second lane mock)
- Policy-locality: WoT settings only affect prioritization (forward/cache order), never object validity decisions

## Release Gates Checklist
- [x] Functional: tag derivation, schema compliance, and ACK behavior (codec + node tests)
- [x] Resilience: packet loss tolerance and cache churn behavior in `veil-sim`
- [x] Performance: record baseline report (p95 latency/throughput/cache hit rate) from `benchmark_runner` (`docs/benchmarks/bench_report_2026-02-06.*`)
- [x] Transport-agnostic: enable CI job with VEIL_E2E_NETWORK=1 for transport smoke test
- [x] Policy-locality: WoT settings only affect prioritization, not validity

## Risks and Mitigations
- **FEC implementation variance** -> lock vectors plus deterministic test corpus
- **Traffic analysis leakage** -> default padding profiles plus bucket normalization
- **Transport coupling** -> strict transport trait boundaries and adapters
- **Cache churn under load** -> simulation-driven eviction tuning before API freeze
