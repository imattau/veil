# VEIL Dart/Flutter SDK

This package provides a Dart/Flutter-facing API and a Rust FFI bridge for the 
VEIL protocol. It enables high-performance protocol operations by wrapping 
the core Rust implementation.

## Goals

- Wrap Rust core crates for tag derivation, encoding, FEC, and crypto.
- Provide Dart-native transport lanes (WebSocket + BLE).
- Provide a client runtime loop and cache policy hooks.
- Keep Flutter apps transport-agnostic.

## Status

- **FFI Bridge**: Full implementation of tag derivation, shard/object metadata 
  decoding, FEC reconstruction, and decryption via `libveil_sdk_bridge`.
- **QUIC Lane**: Native QUIC transport support via FFI (Android/Linux/Windows).
- **WebSocket Lane**: Reconnection + buffered send queue.
- **MultiLane**: Pool multiple lanes for round-robin or broadcast sending.
- **BLE Lane**: FlutterReactiveBle-backed lane with MTU chunking helpers.
- **Client Runtime**: Loop with shard decoding + reconstruction + payload decrypt.
- **Persistence Adapters**: `sqflite` (native) + IndexedDB (web).
- **Cache Eviction**: Rarity-biased (evict most common first, then oldest).
- **Shard Pull Requests**: Request missing shards when `k-1` indices arrive.
- **Overlapping RV Tags**: Helper for epoch transition windows.
- **Blob Manifests**: App-level CBOR helpers for attachment bundles.
- **Social Schemas**: Non-normative app payload helpers (post/media/chunk).
- **Auto-Fetch Plugins**: Subscribe to thread/media references.

## Next steps

1. Add transport health scoring + adaptive lane fanout.
2. Add hop-aware forward limits and shard request budgeting in the runtime loop.
3. Provide optional Wasm builds for Flutter web.

## Multi-lane usage

Use `MultiLane` to pool multiple WebSocket or BLE lanes:

```dart
final lane = MultiLane(
  lanes: [
    WebSocketLane(url: Uri.parse("wss://node-a.example"), peerId: "app"),
    WebSocketLane(url: Uri.parse("wss://node-b.example"), peerId: "app"),
  ],
);
```

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
