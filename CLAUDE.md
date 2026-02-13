# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build, Test, and Lint Commands

All commands run from the repository root.

```bash
# Full CI check (format, lint, build, test)
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo build --workspace --all-targets
cargo test --workspace

# Run a single crate's tests
cargo test -p veil-core
cargo test -p veil-node

# Run a single test by name
cargo test -p veil-codec derives_feed_tag

# E2E transport smoke test (requires network flag)
VEIL_E2E_NETWORK=1 cargo test -p veil-sim --test e2e_transport_runtime_smoke

# Run a simulation example
cargo run -p veil-sim --example runtime_facade

# JS SDK (from repo root)
npm run wasm:build          # Build WASM bindings
npm run sdk:build           # TypeScript compilation
npm run sdk:test            # Vitest unit tests
npm run sdk:verify          # Full verification pipeline
npm run lint                # ESLint
```

## Architecture Overview

VEIL is a shard-native overlay network. Apps encrypt payloads into Objects, which are erasure-coded into Shards and forwarded across multiple transport lanes (QUIC, Tor, WebSocket, BLE). Nodes only see opaque shards -- never semantic messages.

### Crate Dependency Layers (bottom-up)

```
veil-core          Base types, hashing, tag derivation (Tag, ObjectRoot, ShardId, Epoch, Namespace)
    ↑
veil-codec         ObjectV1/ShardV1 CBOR encoding/validation
veil-crypto        XChaCha20-Poly1305 AEAD, Ed25519 signing, secp256k1 Schnorr (Nostr)
    ↑
veil-fec           Reed-Solomon erasure coding (profiles: MICRO k=2/n=3, SMALL k=6/n=10, LARGE k=10/n=16)
    ↑
veil-transport     TransportAdapter trait: send(peer, bytes), recv() -> (peer, bytes)
veil-transport-*   Concrete lanes: QUIC (quinn), WebSocket (tokio-tungstenite), Tor (SOCKS5), BLE (btleplug)
    ↑
veil-node          Runtime, publish/receive pipeline, forwarding, rarity-biased cache, WoT policy, subscriptions
    ↑
veil-sim           E2E tests, performance/stress simulations
veil-wasm          WASM bindings for JS SDK
veil-schema-feed   App-level feed bundle schemas (posts, profiles, reactions, DMs)
```

### Key Design Patterns

- **Node-everywhere:** All participants (mobile, VPS) run the same shard forwarding/caching/reconstruction logic. UIs are thin clients over localhost RPC (HTTP + WebSocket).
- **Tag-based routing:** Public feed tags `H("feed" || pubkey || namespace)` are stable; private rendezvous tags `H("rv" || pubkey || epoch || namespace)` rotate per epoch. Nodes only forward shards matching subscribed tags.
- **Multi-lane delivery:** Fast lane (QUIC) sends k+2 shards to 2 peers; fallback lanes (WS/Tor/BLE) send 2 shards. ACK-driven escalation with bounded retries.
- **Local policy sovereignty:** Web-of-Trust tiers (Trusted/Known/Unknown/Muted/Blocked) control forwarding quotas and cache retention locally. They never affect protocol validity.
- **Rarity-biased cache eviction:** Under pressure, rare shards are retained longer than common ones to preserve reconstruction coverage.

### Applications

- **`apps/veil-vps-node`** -- Production VPS edge forwarder with multi-transport, SQLite settings, Nostr identity, admin web dashboard, peer discovery. Configured via 100+ env vars (prefix `VEIL_VPS_`).
- **`apps/android-node`** -- Android foreground service wrapping Rust node. Per-ABI binaries (arm64-v8a, armeabi-v7a, x86_64) + Kotlin service + Embedded Flutter social feed UI. Build with `scripts/build_android_node.sh`.
- **`apps/veil-desktop`** -- Electron + React desktop app using veil-sdk-js.
- **`apps/veil-desktop-relay`** -- Minimal WebSocket relay for desktop app.

### SDKs

- **`packages/veil-sdk-js`** -- TypeScript SDK (browser/Node/React Native). Tag derivation, lane adapters, storage adapters (IDB, Memory, AsyncStorage), client runtime.
- **`packages/veil-sdk-dart`** -- Dart/Flutter SDK via Flutter Rust Bridge. Rust core at `packages/veil-sdk-dart/rust/`.

## Response Style

Keep responses concise to minimize token usage. Avoid verbose explanations unless asked. Prefer short confirmations, bullet points, and code over prose.

## Coding Conventions

- Standard `rustfmt` with 4-space indentation. `snake_case` functions/modules, `CamelCase` types/traits, `SCREAMING_SNAKE_CASE` constants.
- Keep crate boundaries clean: shared protocol types belong in `veil-core`. No circular dependencies.
- Prefer small, explicit `pub` APIs; keep internals private.
- Unit tests go in `#[cfg(test)]` modules; integration tests in `crates/<crate>/tests/`. Name tests for behavior (e.g., `derives_feed_tag_deterministically`).
- Commits: short imperative subject <= 72 chars, one logical change each.

## Key Reference Docs

- `SPEC.md` -- Normative protocol spec (ObjectV1, ShardV1, tags, forwarding, WoT)
- `ROADMAP.md` -- Implementation phases and milestones
- `docs/node_rpc.md` -- Localhost HTTP+WS RPC contract for node
- `docs/app-schemas/` -- Feed bundle schema definitions
