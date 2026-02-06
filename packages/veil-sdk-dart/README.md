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

## Next steps

1. Generate FRB bindings from a Rust crate that exposes:
   - `derive_feed_tag`, `derive_rv_tag`, `current_epoch` (veil-core)
   - `encode_object`, `decode_object` (veil-codec)
   - `object_to_shards`, `reconstruct_object` (veil-fec)
2. Wire `VeilClient` to call the bridge for validation and reconstruction.
3. Add persistence adapters (Sqflite / drift / IndexedDB).

## Rust bridge (FRB) layout

Rust crate lives at `packages/veil-sdk-dart/rust` and exposes functions
annotated with `#[frb]`. Generate Dart bindings with flutter_rust_bridge:

```bash
# From repo root (example; adjust for your FRB tooling)
cargo build -p veil_sdk_bridge
```

Then run `flutter_rust_bridge_codegen` with your project paths to produce
`lib/src/bridge/veil_bridge.g.dart`.
