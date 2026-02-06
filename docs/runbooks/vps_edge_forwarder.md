# VPS Edge Forwarder + Hot Cache Runbook

This runbook describes how to deploy and operate the VEIL VPS node app
(`veil-vps-node`) in **Edge Forwarder** mode with a **Hot Cache**.

## 1) Build

```bash
cargo build -p veil-vps-node --release
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
export VEIL_VPS_FAST_PEERS=10.0.0.2:5000,10.0.0.3:5000
export VEIL_VPS_CORE_TAGS=
export VEIL_VPS_PEER_LIST_PATH=/opt/veil-vps-node/data/discovered_peers.txt
export VEIL_VPS_MAX_DYNAMIC_PEERS=512

# Optional fallback lanes
export VEIL_VPS_WS_URL=ws://relay.example:8080
export VEIL_VPS_WS_PEER=relay-ws
export VEIL_VPS_TOR_SOCKS_ADDR=127.0.0.1:9050
export VEIL_VPS_TOR_PEERS=peer.onion:5000

# Optional policy knobs
export VEIL_VPS_MAX_CACHE_SHARDS=200000
export VEIL_VPS_BUCKET_JITTER=0
export VEIL_VPS_REQUIRED_SIGNED_NAMESPACES=1,2
export VEIL_VPS_ADAPTIVE_LANE_SCORING=1
export VEIL_VPS_SNAPSHOT_SECS=60
export VEIL_VPS_TICK_MS=50
export VEIL_VPS_HEALTH_PORT=9090
```

Notes:
- QUIC requires trusted certs. If you have peer certs, set
  `VEIL_VPS_QUIC_TRUSTED_CERTS=/path/peer1.der,/path/peer2.der`.
- Tor lane is outbound-only. Use for fallback resilience.

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

Health check & metrics:
- Local HTTP health endpoint: `http://127.0.0.1:9090/health`
- Metrics endpoint: `http://127.0.0.1:9090/metrics`

## 5) Recovery

- State is restored from `VEIL_VPS_STATE_PATH` on boot.
- Delete the CBOR file if you need a cold start.

## 6) Security

- Keep `node_identity.key` and QUIC key material private.
- Restrict filesystem permissions on the data directory.
