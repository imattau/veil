# VPS Edge Forwarder + Hot Cache Runbook

This runbook describes how to deploy and operate the VEIL VPS node app
(`veil-vps-node`) in **Edge Forwarder** mode with a **Hot Cache**.

## 1) Build

```bash
cargo build -p veil-vps-node --release
```

Enable BLE lane (btleplug backend):

```bash
cargo build -p veil-vps-node --release --features ble-btleplug
```

Binary:
- `target/release/veil-vps-node`

## 2) Configure

Create a working directory (example):

```bash
mkdir -p /opt/veil-vps-node/data
```

Set environment variables:

```bash
export VEIL_VPS_STATE_PATH=/opt/veil-vps-node/data/node_state.cbor
export VEIL_VPS_NODE_KEY_PATH=/opt/veil-vps-node/data/node_identity.key
export VEIL_VPS_QUIC_CERT_PATH=/opt/veil-vps-node/data/quic_cert.der
export VEIL_VPS_QUIC_KEY_PATH=/opt/veil-vps-node/data/quic_key.der
export VEIL_VPS_QUIC_BIND=0.0.0.0:5000
export VEIL_VPS_QUIC_ALPN=veil-quic/1,veil/1,veil-node,veil,h3,hq-29
export VEIL_VPS_FAST_PEERS=10.0.0.2:5000,10.0.0.3:5000
export VEIL_VPS_CORE_TAGS=
export VEIL_VPS_PEER_DB_PATH=/opt/veil-vps-node/data/peers.db
export VEIL_VPS_MAX_DYNAMIC_PEERS=512

# Optional fallback lanes
export VEIL_VPS_WS_URL=ws://relay.example:8080
export VEIL_VPS_WS_LISTEN=0.0.0.0:8080
export VEIL_VPS_WS_PEER=relay-ws
export VEIL_VPS_TOR_SOCKS_ADDR=127.0.0.1:9050
export VEIL_VPS_TOR_PEERS=peer.onion:5000
# Optional BLE fallback lane (requires build with --features ble-btleplug)
export VEIL_VPS_BLE_ENABLE=0
export VEIL_VPS_BLE_PEERS=ble-peer-1,ble-peer-2
export VEIL_VPS_BLE_ALLOWLIST=AA:BB:CC:DD:EE:FF
export VEIL_VPS_BLE_MTU=180

# Optional policy knobs
export VEIL_VPS_MAX_CACHE_SHARDS=200000
export VEIL_VPS_BUCKET_JITTER=0
export VEIL_VPS_REQUIRED_SIGNED_NAMESPACES=1,2
export VEIL_VPS_ADAPTIVE_LANE_SCORING=1
export VEIL_VPS_OPEN_RELAY=0
export VEIL_VPS_BLOCKED_PEERS=
export VEIL_VPS_NOSTR_BRIDGE_ENABLE=0
export VEIL_VPS_NOSTR_RELAYS=wss://relay.damus.io,wss://nos.lol,wss://relay.snort.social
export VEIL_VPS_NOSTR_CHANNEL_ID=nostr-bridge
export VEIL_VPS_NOSTR_NAMESPACE=32
export VEIL_VPS_NOSTR_SINCE_SECS=3600
export VEIL_VPS_NOSTR_BRIDGE_STATE_PATH=/opt/veil-vps-node/data/nostr-bridge-state.json
export VEIL_VPS_NOSTR_MAX_SEEN_IDS=20000
export VEIL_VPS_NOSTR_PERSIST_EVERY_UPDATES=32
export VEIL_VPS_SNAPSHOT_SECS=60
export VEIL_VPS_TICK_MS=50
export VEIL_VPS_HEALTH_PORT=9090
```

Notes:
- QUIC requires trusted certs. If you have peer certs, set
  `VEIL_VPS_QUIC_TRUSTED_CERTS=/path/peer1.der,/path/peer2.der`.
- Tor lane is outbound-only. Use for fallback resilience.
- BLE lane uses btleplug when compiled with `--features ble-btleplug`.
- `VEIL_VPS_OPEN_RELAY=1` makes the node accept all tags and remove non-blocked WoT forwarding throttles.
- `VEIL_VPS_BLOCKED_PEERS` still hard-blocks listed peers by peer id.
- `VEIL_VPS_NOSTR_BRIDGE_ENABLE=1` starts a relay bridge that maps Nostr `kind:1` events to VEIL post bundles.

## 3) Run

```bash
target/release/veil-vps-node
```

## 4) Observe

The node logs periodic lane health snapshots and persistence errors (if any).
You can tail systemd logs or stdout for:

- transport health counters
- snapshot write failures
- adapter startup errors

Health check, metrics, peers, admin:
- Local HTTP health endpoint: `http://127.0.0.1:9090/health` (bind configurable via `VEIL_VPS_HEALTH_BIND`)
- Metrics endpoint: `http://127.0.0.1:9090/metrics`
- Peers endpoint: `http://127.0.0.1:9090/peers`
  - Optional query params: `limit` (max 1000), `prefix` (e.g., `ws:`, `wssrv:`, `tor:`, `ble:`)
- Admin API: `http://127.0.0.1:9090/admin-api/status`, `/login`, `/settings`, etc.

## 5) Recovery

- State is restored from `VEIL_VPS_STATE_PATH` on boot.
- Delete the CBOR file if you need a cold start.

## 6) Security

- Keep `node_identity.key` and QUIC key material private.
- Restrict filesystem permissions on the data directory.
