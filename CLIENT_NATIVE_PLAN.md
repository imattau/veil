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
- [ ] Expose shard/object encode/decode helpers in wasm.
- [ ] Implement full transport adapters (WebSocket/WebRTC lanes with reconnect/backoff).
- [ ] Expand SDK `VeilClient` runtime loop (`ingest`, `forward`, `subscribe`, callback hooks) beyond scaffold.
- [ ] Persist cache metadata to IndexedDB.
- [ ] Add React Native persistent cache adapter (AsyncStorage/MMKV/SQLite).

## Phase 3: Production hardening
- [ ] Key management helpers (WebCrypto-based, safe defaults).
- [ ] Reconnect/backoff and lane health scoring.
- [ ] Browser e2e tests for loss/duplicate/tamper behavior.
- [ ] Package publishing flow for `@veil/sdk-js`.

## Acceptance checks
- React demo derives feed + rendezvous tags entirely client-side.
- SDK can run in browser (wasm) and React Native (pure-js) contexts.
- Rust workspace and CI still pass (`fmt`, `clippy`, `build`, `test`).
