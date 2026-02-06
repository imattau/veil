# VEIL Desktop (Electron + React)

This is a Linux desktop example that connects to a VEIL WebSocket lane using the
JavaScript SDK. It can optionally spawn a local relay so the app is self‑contained.

## Build & Run

```bash
cd apps/veil-desktop
npm install
npm --prefix ../../packages/veil-sdk-js run build
```

## Run (dev)

```bash
npm run dev
```

## Self-contained mode (local relay)

Build the relay binary:

```bash
cargo build -p veil-desktop-relay --release
```

Run the app with the relay path:

```bash
VEIL_DESKTOP_RELAY_PATH=../../target/release/veil-desktop-relay npm run dev
```

The app will spawn the relay and connect to `ws://127.0.0.1:9001` by default.

## Packaged build (bundled relay)

```bash
cargo build -p veil-desktop-relay --release
npm run package
```

The packaging script copies the relay binary into `dist/relay/veil-desktop-relay`.
Set `VEIL_DESKTOP_RELAY_PATH` to that bundled path at runtime if needed.

## Notes

- Uses the SDK’s `WebSocketLaneAdapter` and `VeilClient`.
- The local relay is a simple broadcast relay (no persistence).
