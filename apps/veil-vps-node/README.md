# VEIL VPS Node (Edge Forwarder + Hot Cache)

This app runs a production-oriented VEIL node using the **Edge Forwarder** profile
with a **Hot Cache** configuration. It wires QUIC as the fast lane and optional
WebSocket + Tor SOCKS5 as fallback lanes. BLE fallback is available when built
with `--features ble-btleplug`.

## Build & Run

```bash
cargo run -p veil-vps-node
```

Enable BLE lane (btleplug backend):

```bash
cargo run -p veil-vps-node --features ble-btleplug
```

## Docker Compose

```bash
docker compose -f apps/veil-vps-node/docker-compose.yml up -d --build
```

Notes:
- Exposes UDP `5000` (QUIC) and TCP `9090` (health/metrics/peers).
- Use `PROXY_DOMAIN` (or proxy-specific env vars) to hint reverse proxy presence.

Enable built-in Caddy reverse proxy (only if you need it):

```bash
docker compose -f apps/veil-vps-node/docker-compose.yml --profile proxy up -d --build
```

Copy the compose env template:

```bash
cp apps/veil-vps-node/.env.example apps/veil-vps-node/.env
```

## Environment

Required:
- `VEIL_VPS_QUIC_BIND` (default `0.0.0.0:5000`)

Optional:
- `VEIL_VPS_STATE_PATH` (default `data/veil-vps-node-state.cbor`)
- `VEIL_VPS_NODE_KEY_PATH` (default `data/node_identity.key`)
- `VEIL_VPS_QUIC_CERT_PATH` (default `data/quic_cert.der`)
- `VEIL_VPS_QUIC_KEY_PATH` (default `data/quic_key.der`)
- `VEIL_VPS_QUIC_TRUSTED_CERTS` (comma-separated cert DER paths)
- `VEIL_VPS_FAST_PEERS` (comma-separated `host:port` for QUIC peers)
- `VEIL_VPS_CORE_TAGS` (comma-separated 64-char hex tags to auto-subscribe)
- `VEIL_VPS_PEER_DB_PATH` (path to persist discovered peers)
- `VEIL_VPS_MAX_DYNAMIC_PEERS` (cap for discovered peers added to fanout)
- `VEIL_VPS_WS_URL` (e.g. `ws://host:port`)
- `VEIL_VPS_WS_PEER` (peer id label used by WebSocket adapter)
- `VEIL_VPS_TOR_SOCKS_ADDR` (e.g. `127.0.0.1:9050`)
- `VEIL_VPS_TOR_PEERS` (comma-separated `host:port` destination peers)
- `VEIL_VPS_BLE_ENABLE` (`1`/`0`, default `0`, requires `--features ble-btleplug`)
- `VEIL_VPS_BLE_PEERS` (comma-separated BLE peer ids/addresses)
- `VEIL_VPS_BLE_ALLOWLIST` (comma-separated BLE adapter addresses to accept)
- `VEIL_VPS_BLE_MTU` (default `180`)
- `VEIL_VPS_SNAPSHOT_SECS` (default `60`)
- `VEIL_VPS_TICK_MS` (default `50`)
- `VEIL_VPS_HEALTH_PORT` (default `9090`, set `0` to disable `/health`, `/metrics`, and `/peers`)
- `VEIL_VPS_MAX_CACHE_SHARDS` (default `200000`)
- `VEIL_VPS_BUCKET_JITTER` (default `0`)
- `VEIL_VPS_REQUIRED_SIGNED_NAMESPACES` (comma-separated namespace ids)
- `VEIL_VPS_ADAPTIVE_LANE_SCORING` (`1`/`0`, default `1`)

## Notes
- QUIC requires trusted peer certificates. Provide peer certs via
  `VEIL_VPS_QUIC_TRUSTED_CERTS` if you expect to connect to other nodes.
- WebSocket is best-effort outbound; Tor SOCKS5 is outbound-only in this profile.
- BLE fallback uses btleplug when the `ble-btleplug` feature is enabled.
- `/peers` supports optional query params: `limit` (max 1000), `prefix` (e.g., `ws:`, `tor:`, `ble:`).
