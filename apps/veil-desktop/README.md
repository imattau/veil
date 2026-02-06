# VEIL Desktop (Electron + React)

This is a Linux desktop example that connects to a VEIL WebSocket lane using the
JavaScript SDK. It is intentionally minimal: a control panel on the left and
real‑time shard activity on the right.

## Prerequisites

- Node.js 18+
- A running VEIL WebSocket relay or node (e.g. `ws://127.0.0.1:9001`)

## Install

```bash
cd apps/veil-desktop
npm install
```

If you change the SDK, rebuild it before running the desktop app:

```bash
npm --prefix ../../packages/veil-sdk-js run build
```

## Run (dev)

```bash
npm run dev
```

This starts Vite and Electron. The app loads `ELECTRON_START_URL` pointing at the
Vite dev server.

## Usage

- Set the WebSocket URL of your VEIL relay/node.
- Provide a peer ID string that matches your relay config.
- Enter forward peers (comma-separated) for fanout targets.
- Subscribe to the tag hex your publisher uses.
- Click **Start**.

## Notes

- This example uses the SDK’s `WebSocketLaneAdapter` and `VeilClient`.
- BLE/QUIC/Tor lanes are not available in Electron; they are Rust runtime lanes.
