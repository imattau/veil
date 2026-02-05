# VEIL

![VEIL Header](veil_header.png)

VEIL is a transport-agnostic, shard-native overlay for censorship-resistant public feeds and privacy-preserving delivery.  
Instead of routing semantic messages, VEIL routes opaque erasure-coded shards across independent network lanes.

## Project aims

VEIL is designed to be practical first:

- **Resilient propagation:** keep public feeds available under packet loss, congestion, and partial censorship.
- **Privacy-preserving delivery:** use rotating rendezvous tags and opaque shard transport for private delivery patterns.
- **Transport independence:** run the same core protocol over multiple lanes (for example QUIC + Tor/WebRTC) without changing shard/object formats.
- **Fast deployability:** provide a usable implementation that does not require a full mixnet to operate effectively.
- **Policy-local operation:** apply trust/payment policies to local resource allocation (forwarding, caching), not protocol validity.

## Repository layout

- `SPEC.md` - protocol/library spec draft (`ObjectV1`, `ShardV1`)
- `ROADMAP.md` - implementation phases and milestones
- `crates/veil-core` - core types, hashing, tag derivation
- `crates/veil-codec` - object/shard encoding and validation
- `crates/veil-crypto` - AEAD and signing interfaces
- `crates/veil-fec` - profile selection and erasure-coding sharder
- `crates/veil-node` - runtime, forwarding, cache, ACK handling
- `crates/veil-transport` - transport adapter abstractions
- `crates/veil-transport-quic` - QUIC fast-lane `TransportAdapter`
- `crates/veil-transport-websocket` - websocket `TransportAdapter` implementation
- `crates/veil-transport-tor` - Tor SOCKS5 fallback `TransportAdapter`
- `crates/veil-sim` - e2e, performance, stress, and memory tests
- `crates/veil-schema-feed` - optional, non-normative app/feed bundle schema
- `docs/runbooks/transport_profiles.md` - deployment notes for fast/fallback lane profiles

## Quick start

```bash
cargo test --workspace
```

## Developer helpers

High-level helper APIs in `veil-node` simplify integration:

- `service::PublisherRuntime` - queue + publish tick wrapper
- `service::NodeRuntime` - ingest/forward/reconstruct tick wrapper
- `service::NodeRuntime::run_steps/run_until` - built-in runtime loop orchestration
- `service::NodeRuntimeCallbacks` - delivery/ACK/send-failure hooks per tick
- `service::NodeRuntimeCallbacks::on_endorsement_ingested` - callback for accepted WoT endorsement payloads
- `persistence` helpers - snapshot/load `NodeState` as CBOR for restart durability
- `policy::LocalWotPolicy::score_publisher/explain_publisher` - deterministic trust scoring with explainability
- transport adapters expose unified `health_snapshot()` and adapter-level `metrics_snapshot()` counters
- `publish::PublishOptions` - typed publish flags (`signed`, `ack_requested`)
- `config::NodeRuntimeConfig::builder()` - fluent runtime configuration
- `veil_fec::sharder::object_to_shards_with_mode(...)` - optional hardened non-systematic FEC mode for high-privacy deployments
- `config::NodeRuntimeConfig::builder().bucket_jitter_extra_levels(...)` - optional upward bucket jitter to blur packet-size fingerprints
- `veil_transport::adapter::route_in_memory_outbound(...)` - move captured in-memory outbound traffic into another adapter's inbound queue for simulations/tests

Run the facade example:

```bash
cargo run -p veil-sim --example runtime_facade
```

Profile examples:

```bash
cargo run -p veil-sim --example edge_forwarder_hot_cache
cargo run -p veil-sim --example bootstrap_peer
cargo run -p veil-sim --example transport_multi_lane_runtime
```

- `edge_forwarder_hot_cache`: edge forwarder + hot cache defaults for VPS-style hosts.
- `bootstrap_peer`: minimal bootstrap/discovery profile with conservative forwarding.

Benchmark runner (writes JSON + CSV):

```bash
cargo run -p veil-sim --bin benchmark_runner -- --quick
```

Outputs:
- `target/benchmarks/veil-sim/bench_report.json`
- `target/benchmarks/veil-sim/bench_report.csv`

Local QUIC network benchmark (real UDP/QUIC sockets):

```bash
cargo run -p veil-sim --bin network_benchmark_quic -- --count 512 --payload-bytes 16384
```

Outputs:
- `target/benchmarks/veil-sim/quic_network_report.json`
- `target/benchmarks/veil-sim/quic_network_report.csv`

## Client-native Node.js / React / React Native

This repo now includes a client-native foundation for JS frontends:

- Rust wasm bindings: `crates/veil-wasm`
- JS SDK scaffold: `packages/veil-sdk-js`
- React demo scaffold: `apps/react-demo`
- React Native scaffold notes: `apps/react-native-demo`
- Implementation plan: `CLIENT_NATIVE_PLAN.md`

SDK now includes:
- backend-aware tag derivation (`wasm` or `pure-js`)
- channel-scoped tag helpers (`deriveChannelNamespace`, `deriveChannelFeedTagHex`)
- shard/object meta decode helpers (`decodeShardMeta`, `decodeObjectMeta`)
- CBOR validation helpers (`validateShardCbor`, `validateObjectCbor`)
- subscription-gated forwarding in `VeilClient` runtime scaffold
- adaptive lane health scoring (`VeilClient.getLaneHealth()` + automatic fanout rebalance)
- `WebSocketLaneAdapter` with reconnect/backoff + bounded send buffering
- `WebRtcLaneAdapter` with reconnect/backoff + buffered sends
- persistent cache adapters:
  - `IndexedDbShardCacheStore` (browser)
  - `AsyncKeyValueShardCacheStore` (React Native AsyncStorage/MMKV wrapper)
- WebCrypto key helpers (`randomBytes`, `hkdfSha256`, `generateEd25519KeyPair`, `signEd25519`, `verifyEd25519`)
- SDK e2e behavior tests for loss/duplicate/tamper ingest paths

WebSocket lane example (browser/React Native global WebSocket, or inject one in Node):

```ts
import { WebSocketLaneAdapter } from "@veil/sdk-js";

const lane = new WebSocketLaneAdapter({
  url: "wss://relay.example/ws",
  peerId: "relay-a",
  autoReconnect: true,
  reconnectInitialMs: 250,
  reconnectMaxMs: 10_000,
  maxBufferedMessages: 256,
});
```

React Native cache example:

```ts
import AsyncStorage from "@react-native-async-storage/async-storage";
import { AsyncKeyValueShardCacheStore } from "@veil/sdk-js";

const cache = new AsyncKeyValueShardCacheStore(AsyncStorage, {
  keyPrefix: "veil:shard:",
});
```

Key helper example:

```ts
import { generateEd25519KeyPair, hkdfSha256, randomBytes } from "@veil/sdk-js";

const signingKeys = await generateEd25519KeyPair();
const payloadKey = await hkdfSha256(randomBytes(32));
```

Build wasm + SDK:

```bash
npm install
npm run wasm:build
npm run sdk:build
```

Verify SDK package readiness:

```bash
npm run sdk:verify
```

CI publish flow uses `npm run sdk:verify:ci` (includes `wasm-pack` build).

Run React demo:

```bash
npm run react:dev
```

React Native note:

- SDK supports `auto`/`wasm`/`pure-js` backend selection.
- Use `pure-js` mode in React Native:

```ts
import { configureTagBackend } from "@veil/sdk-js";
configureTagBackend("pure-js");
```

## SDK publishing

`@veil/sdk-js` publish flow is automated via `.github/workflows/publish-sdk-js.yml`.

- Push tag `sdk-js-vX.Y.Z` (must match `packages/veil-sdk-js/package.json` version) to publish.
- Or run the workflow manually with `dry_run` first.
- Workflow verifies with `npm run sdk:verify:ci` before publishing.
- Configure repository secret `NPM_TOKEN` for npm publish access.

## Optional fuzzing

```bash
cargo install cargo-fuzz
cargo fuzz run codec_decode --manifest-path fuzz/Cargo.toml
cargo fuzz run node_runtime_ingest --manifest-path fuzz/Cargo.toml
```
