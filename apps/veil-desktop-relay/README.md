# VEIL Desktop Relay

Minimal local WebSocket relay used by the Electron desktop example. It broadcasts
binary payloads to all connected clients.

## Run

```bash
cargo run -p veil-desktop-relay
```

## Configuration

- `VEIL_RELAY_BIND` (default `127.0.0.1:9001`)
