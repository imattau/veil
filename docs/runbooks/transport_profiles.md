# Transport Profiles Runbook

This runbook documents a practical multi-lane setup for VEIL runtime deployments.

## Profile

- **Fast lane:** WebSocket transport (`veil-transport-websocket`)
- **Fallback lane:** Tor SOCKS5 transport (`veil-transport-tor`)

Recommended use:
- VPS/public nodes: keep fast lane stable and low-latency.
- Censorship-sensitive fallback: route overflow and retry traffic through Tor.

## Runtime Example

Use the example profile runner:

```bash
cargo run -p veil-sim --example transport_multi_lane_runtime
```

Environment variables:

- `VEIL_FAST_WS_URL` (default `ws://127.0.0.1:9001`)
- `VEIL_FALLBACK_SOCKS_PROXY` (default `127.0.0.1:9050`)
- `VEIL_FAST_PEERS` (comma-separated, default `fast-peer`)
- `VEIL_FALLBACK_PEERS` (comma-separated, default `fallback-peer`)

## Tor Setup

Run local Tor daemon exposing SOCKS5 on `127.0.0.1:9050` (or set custom proxy endpoint).

Operational checks:

1. Confirm SOCKS endpoint is reachable before starting VEIL.
2. Keep Tor connect/send timeouts conservative to avoid runtime stalls.
3. Use bounded queue capacities to cap memory use under outage conditions.

## Production Notes

- Prefer `NodeRuntime::run_until` for service processes with cancellation control.
- Snapshot state periodically via `veil-node::persistence` helpers for restart continuity.
- Treat Tor lane as best-effort fallback; fast lane should carry most traffic.
