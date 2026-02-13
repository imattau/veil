# VEIL Android Node

This app provides an Android foreground service that wraps the VEIL Rust node. It 
exposes a local RPC interface (HTTP + WebSocket) for thin clients, such as the 
embedded Flutter UI.

## Architecture

- **Foreground Service**: Manages the node lifecycle, ensuring it stays alive to 
  drain publish queues and receive shards in the background.
- **Rust Node**: The core VEIL protocol implementation, running as a native 
  binary or library linked via JNI.
- **Local RPC**: Authenticated loopback interface (`127.0.0.1`) for UI communication.
- **Embedded UI**: Flutter-based social feed application located in `src/ui`.

## Build

### Native Node Binaries

Build the per-ABI binaries and copy them to Android assets:

```bash
./scripts/build_android_node.sh
```

### Android App

Build the APK or run on a device:

```bash
cd apps/android-node/src/ui
flutter build apk
# or
flutter run
```

## Configuration

The foreground service configures the node via environment variables:
- `VEIL_NODE_TOKEN`: Local RPC authentication token.
- `VEIL_NODE_CACHE_STATE`: Path to persist shard cache.
- `VEIL_NODE_QUIC_BIND`: Local address for QUIC listener.
- `VEIL_NODE_STATE_KEY_HEX`: Key for state encryption (derived from Android KeyStore).

See `docs/node_rpc.md` for the local API contract.
