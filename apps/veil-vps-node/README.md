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

## Settings CLI

Manage runtime settings in `data/settings.db`:

```bash
veil-vps-node settings list
veil-vps-node settings get VEIL_VPS_OPEN_RELAY
veil-vps-node settings set VEIL_VPS_OPEN_RELAY 1
veil-vps-node settings delete VEIL_VPS_OPEN_RELAY
```

Use a custom DB path:

```bash
veil-vps-node settings --db /opt/veil-vps-node/data/settings.db list
```

## Docker Compose

```bash
docker compose -f apps/veil-vps-node/docker-compose.yml up -d --build
```

Notes:
- Exposes UDP `5000` (QUIC). Health endpoints bind to `127.0.0.1` by default.
- Use `PROXY_DOMAIN` (or proxy-specific env vars) to hint reverse proxy presence.

Enable built-in Caddy reverse proxy (only if you need it):

```bash
docker compose -f apps/veil-vps-node/docker-compose.yml --profile proxy up -d --build
```

When running with the proxy profile, the VPS node serves a landing page at
`http://<your-domain>/` with a VEIL overview and a QR code for app onboarding.
An admin page is available at `http://<your-domain>/admin/` and authenticates
using the node's Nostr identity secret (`VEIL_VPS_NODE_KEY_PATH` key as `nsec` or hex).
The admin page can also manage settings in `data/settings.db` (list/get/set/delete).

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
- `VEIL_VPS_QUIC_ALPN` (comma-separated ALPN list to advertise; overrides `VEIL_QUIC_ALPN`)
- `VEIL_VPS_QUIC_CERT_PATH` (default `data/quic_cert.der`)
- `VEIL_VPS_QUIC_KEY_PATH` (default `data/quic_key.der`)
- `VEIL_VPS_QUIC_TRUSTED_CERTS` (comma-separated cert DER paths)
- `VEIL_VPS_FAST_PEERS` (comma-separated `host:port` for QUIC peers)
- `VEIL_VPS_CORE_TAGS` (comma-separated 64-char hex tags to auto-subscribe)
- `VEIL_VPS_PEER_DB_PATH` (path to persist discovered peers)
- `VEIL_VPS_MAX_DYNAMIC_PEERS` (cap for discovered peers added to fanout)
- `VEIL_VPS_WS_URL` (e.g. `ws://host:port`)
- `VEIL_VPS_WS_LISTEN` (e.g. `0.0.0.0:8080`, enables inbound WebSocket lane)
- `VEIL_VPS_WS_PEER` (peer id label used by WebSocket adapter)
- `VEIL_VPS_TOR_SOCKS_ADDR` (e.g. `127.0.0.1:9050`)
- `VEIL_VPS_TOR_PEERS` (comma-separated `host:port` destination peers)
- `VEIL_VPS_BLE_ENABLE` (`1`/`0`, default `0`, requires `--features ble-btleplug`)
- `VEIL_VPS_BLE_PEERS` (comma-separated BLE peer ids/addresses)
- `VEIL_VPS_BLE_ALLOWLIST` (comma-separated BLE adapter addresses to accept)
- `VEIL_VPS_BLE_MTU` (default `180`)
- `VEIL_VPS_SNAPSHOT_SECS` (default `60`)
- `VEIL_VPS_TICK_MS` (default `50`)
- `VEIL_VPS_HEALTH_BIND` (default `127.0.0.1`)
- `VEIL_VPS_HEALTH_PORT` (default `9090`, set `0` to disable `/health`, `/metrics`, and `/peers`)
- `VEIL_VPS_ADMIN_SESSION_DB_PATH` (default `data/admin-sessions.db`)
- `VEIL_VPS_MAX_CACHE_SHARDS` (default `200000`)
- `VEIL_VPS_BUCKET_JITTER` (default `0`)
- `VEIL_VPS_REQUIRED_SIGNED_NAMESPACES` (comma-separated namespace ids)
- `VEIL_VPS_ADAPTIVE_LANE_SCORING` (`1`/`0`, default `1`)
- `VEIL_VPS_OPEN_RELAY` (`1`/`0`, default `0`; accept all tags and forward all non-blocked peers)
- `VEIL_VPS_BLOCKED_PEERS` (comma-separated peer ids to hard-block, e.g. `1.2.3.4:5000,ws:relay-a`)
- `VEIL_VPS_NOSTR_BRIDGE_ENABLE` (`1`/`0`, default `0`)
- `VEIL_VPS_NOSTR_RELAYS` (comma-separated relay URLs, e.g. `wss://relay.damus.io,wss://nos.lol`)
- `VEIL_VPS_NOSTR_CHANNEL_ID` (default `nostr-bridge`)
- `VEIL_VPS_NOSTR_NAMESPACE` (default `32`)
- `VEIL_VPS_NOSTR_SINCE_SECS` (default `3600`)
- `VEIL_VPS_NOSTR_BRIDGE_STATE_PATH` (default `data/nostr-bridge-state.json`)
- `VEIL_VPS_NOSTR_MAX_SEEN_IDS` (default `20000`)
- `VEIL_VPS_NOSTR_PERSIST_EVERY_UPDATES` (default `32`)

## Notes
- Settings are loaded from SQLite only (`data/settings.db`).
- On first run, if the DB is empty and `/opt/veil-vps-node/veil-vps-node.env` exists,
  values are imported once for migration.
- QUIC requires trusted peer certificates. Provide peer certs via
  `VEIL_VPS_QUIC_TRUSTED_CERTS` if you expect to connect to other nodes.
- `VEIL_VPS_NODE_KEY_PATH` stores a Nostr-compatible secp256k1 secret key
  (32 bytes), also used as the node decrypt key.
- WebSocket is best-effort outbound; Tor SOCKS5 is outbound-only in this profile.
- BLE fallback uses btleplug when the `ble-btleplug` feature is enabled.
- `/peers` supports optional query params: `limit` (max 1000), `prefix` (e.g., `ws:`, `wssrv:`, `tor:`, `ble:`).
- When installed via the installer, the landing page and `/health`, `/metrics`, `/peers`
  are publicly readable through the reverse proxy by default.
