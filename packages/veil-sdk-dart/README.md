# VEIL Dart/Flutter SDK (Scaffold)

This package provides a Dart/Flutter-facing API and a Rust FFI bridge scaffold
for the VEIL protocol. The current implementation focuses on structure and
interfaces; the Rust bridge functions are stubbed for now.

## Goals

- Wrap Rust core crates for tag derivation, encoding, FEC, and crypto.
- Provide Dart-native transport lanes (WebSocket + BLE).
- Provide a client runtime loop and cache policy hooks.
- Keep Flutter apps transport-agnostic.

## Status

- FFI bridge: scaffolded API surface in `lib/src/bridge/veil_bridge.dart`.
- WebSocket lane: reconnection + buffered send queue.
- BLE lane: FlutterReactiveBle-backed lane with MTU chunking helpers.
- Client runtime: loop with shard decoding + reconstruction + payload decrypt.
- Persistence adapters: sqflite + IndexedDB (web).
- Cache eviction: rarity-biased (evict most common first, then oldest).
- Shard pull requests: request missing shards when `k-1` indices arrive.
- Overlapping RV tags: helper for epoch transition windows.
- Blob manifests: app-level CBOR helpers for attachment bundles.
- Social schemas: non-normative app payload helpers (post/media/chunk).
- Auto-fetch plugins: subscribe to thread/media references.

## Next steps

1. Add transport health scoring + adaptive lane fanout.
2. Add hop-aware forward limits and shard request budgeting in the runtime loop.
3. Provide optional Wasm builds for Flutter web.

## Web support

The FRB bridge generates web bindings, but you must build the Rust crate to Wasm
for Flutter Web usage. Without that, `VeilBridge.init()` will fail in browsers.

## Rust bridge (FRB) layout

Rust crate lives at `packages/veil-sdk-dart/rust` and exposes functions
annotated with `#[frb]`. Generate Dart bindings with flutter_rust_bridge:

```bash
# From repo root (example; adjust for your FRB tooling)
cargo build -p veil_sdk_bridge
```

Then run `flutter_rust_bridge_codegen` with your project paths to produce
`lib/src/bridge/veil_bridge.g.dart`.
