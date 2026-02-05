# Client-Native Node.js/React/React-Native Plan

## Goal
Deliver a browser/Node/React-Native capable VEIL client path that does not require an HTTP gateway for core primitives, and can evolve into full shard runtime support.

## Phase 1 (Implemented now): Foundation
- [x] Add a Rust `veil-wasm` crate for browser/Node bindings.
- [x] Export JS-safe primitives:
  - [x] `deriveFeedTag`
  - [x] `deriveRvTag`
  - [x] `currentEpoch`
  - [x] `bytesToHex`
- [x] Add a JS SDK package scaffold (`packages/veil-sdk-js`).
- [x] Add a React demo app scaffold (`apps/react-demo`) using the SDK.
- [x] Add backend selection (`auto` / `wasm` / `pure-js`) for environments where wasm is not practical (React Native).
- [x] Add transport + storage scaffolds in SDK (`WebSocketLaneAdapter`, `ShardCacheStore`).
- [x] Add React Native integration scaffold notes (`apps/react-native-demo`).
- [x] Document build/dev scripts for wasm + SDK + React demo.

## Phase 2: Client runtime MVP (next)
- [x] Expose shard/object decode + validation helpers in wasm.
- [x] Implement WebSocket lane with reconnect/backoff, buffered sends, and injectable socket factory for browser/Node/React Native environments.
- [x] Implement WebRTC lane adapter with reconnect/backoff and buffered sends.
- [x] Expand SDK `VeilClient` runtime loop (`ingest`, `forward`, `subscribe`, callback hooks) for subscription-gated forwarding and hot-cache writes.
- [x] Persist cache metadata to IndexedDB via `IndexedDbShardCacheStore`.
- [x] Add React Native persistent cache adapter interface via `AsyncKeyValueShardCacheStore` (for AsyncStorage/MMKV wrappers).

## Phase 3: Production hardening
- [x] Key management helpers (WebCrypto-based, safe defaults).
- [x] Reconnect/backoff and lane health scoring.
- [x] Browser e2e tests for loss/duplicate/tamper behavior.
- [x] Package publishing flow for `@veil/sdk-js`.

## Acceptance checks
- React demo derives feed + rendezvous tags entirely client-side.
- SDK can run in browser (wasm) and React Native (pure-js) contexts.
- Rust workspace and CI still pass (`fmt`, `clippy`, `build`, `test`).
