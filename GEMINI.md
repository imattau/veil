# VEIL Project Context

VEIL is a transport-agnostic, shard-native overlay for censorship-resistant public feeds and privacy-preserving delivery. It routes opaque, erasure-coded shards across independent network lanes (QUIC, Tor, WebSocket, BLE) rather than semantic messages.

## Project Overview

- **Core Concept:** Objects (application units) are encrypted and split into fixed-size Shards (network units) using Reed-Solomon erasure coding.
- **Architecture:** "Node-Everywhere" model where all participants (mobile, VPS) implement the same shard forwarding, caching, and reconstruction logic.
- **Discovery:** Tag-based routing using stable feed tags (public) and rotating rendezvous tags (private, epoch-based).
- **Resource Policy:** Local Web-of-Trust (WoT) tiers (Trusted, Known, Unknown, Muted, Blocked) manage forwarding quotas and cache retention locally without affecting global protocol validity.

## Main Technologies

- **Core Logic (Rust):** `veil-core`, `veil-codec`, `veil-crypto`, `veil-fec`, `veil-node`.
- **Transports:** QUIC (`quinn`), WebSocket (`tokio-tungstenite`), Tor (SOCKS5), BLE (`btleplug`).
- **Applications:**
    - **Android:** Flutter UI over an embedded Rust node service (`apps/android-node`, `apps/veil_android`).
    - **Desktop:** Electron + React (`apps/veil-desktop`).
    - **VPS:** Production edge forwarder (`apps/veil-vps-node`).
- **SDKs:** JS/TypeScript (`packages/veil-sdk-js`) and Dart/Flutter (`packages/veil-sdk-dart`).
- **Data Formats:** CBOR (encoding), BLAKE3 (hashing), XChaCha20-Poly1305 (AEAD).

## Building and Running

### Rust (Core & Node)
- **Test Workspace:** `cargo test --workspace`
- **Build Workspace:** `cargo build --workspace --all-targets`
- **Run Simulation Example:** `cargo run -p veil-sim --example runtime_facade`
- **Lint/Check:** `cargo clippy --workspace --all-targets` and `cargo fmt --all -- --check`

### JS SDK & Web Demo
- **Build WASM Bindings:** `npm run wasm:build`
- **Build SDK:** `npm run sdk:build`
- **Run SDK Tests:** `npm run sdk:test`
- **Run React Demo:** `npm run react:dev`

### Android Node
- **Build Per-ABI Binaries:** `./scripts/build_android_node.sh` (copies binaries to Android assets)

## Development Conventions

- **Crate Dependencies:** Maintain strict layering (Core -> Codec -> FEC -> Transport -> Node).
- **Localhost RPC:** UIs are thin clients that communicate with the local node via HTTP/WebSocket on `127.0.0.1`. See `docs/node_rpc.md` for the contract.
- **Testing:** Unit tests in `#[cfg(test)]` modules; integration tests in `tests/` directories; E2E simulations in `crates/veil-sim`.
- **Identity:** The node process owns identity keys, shard cache, and publish queues.
- **Coding Style:** Standard Rust `fmt` and `clippy` rules; `snake_case` for functions, `CamelCase` for types.

## Key Files

- `MASTER_PLAN.md`: Consolidated implementation plan and task status across all project features.
- `SPEC.md`: Normative protocol and library specification (ObjectV1, ShardV1, Tags).
- `ROADMAP.md`: Staged implementation plan and current milestone status.
- `CLAUDE.md`: Detailed developer guide with common commands and architecture maps.
- `docs/node_rpc.md`: Local node API contract (HTTP + WebSocket).
- `docs/app-schemas/SOCIAL_SCHEMAS.md`: Application-level feed bundle definitions.

## Universal Actions
- Use Groq for offloading unit testing and generation, real-time documentation search, and batch formatting to improve performance and responsiveness.
