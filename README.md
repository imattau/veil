# VEIL

![VEIL Header](veil_header.png)

VEIL is a transport-agnostic, shard-native overlay for censorship‑resistant public feeds and privacy‑preserving delivery. Instead of routing semantic messages, VEIL routes opaque, erasure‑coded shards across independent network lanes (QUIC, Tor, WebRTC, BLE, WebSocket). This keeps delivery resilient under loss, throttling, and partial censorship—without requiring a full mixnet.

## How VEIL works (conceptual)

**Objects → Shards**
- Apps encrypt payloads into **ObjectV1** and then split into fixed‑size **ShardV1** buckets.
- Shards are opaque blobs; nodes forward, cache, and dedupe by `shard_id` (hash of shard bytes).

**Tags for discovery**
- **Public feeds** use a stable feed tag: `H("feed" || publisher_pubkey || namespace)`.
- **Private rendezvous** uses rotating tags: `H("rv" || recipient_pubkey || epoch || namespace)`.
- Nodes subscribe to tags; only shards with subscribed tags are forwarded.

**Multi‑lane delivery**
- The same shard format travels over multiple transports (“lanes”).
- Lanes are local policy—no lane identity is embedded in shard headers.

**Reconstruction**
- When `k` unique shards arrive for the same `object_root`, the object is reconstructed, decrypted, and delivered to the app.

**Resource policy is local**
- VEIL does not enforce global ranking. Nodes decide how to allocate bandwidth and cache using local policy (e.g., Web‑of‑Trust tiers, payment proofs, or simple heuristics).

## What’s unique about VEIL

- **Shard‑native overlay:** the network sees only opaque shards, never semantic messages.
- **Transport independence:** identical shard/object formats across QUIC/Tor/WebRTC/BLE/WebSocket lanes.
- **Practical privacy:** rotating rendezvous tags + optional overlapping epochs for private delivery.
- **Loss tolerance:** erasure coding enables reconstruction without full delivery.
- **Policy‑local:** trust and payment signals influence local resource allocation, not protocol validity.
- **Hardened FEC default:** non‑systematic encoding by default to avoid “first‑k” ciphertext exposure.
- **Systematic public feed mode:** namespace `1` may use systematic encoding for lower ingest overhead.
- **Traffic shaping:** optional bucket jitter to blur size fingerprints.
- **Efficiency controls:** optional Bloom-filter shard summaries + rarity-biased probabilistic forwarding.

## Core schemas (summary)

**ObjectV1 (application unit, pre‑sharding)**
- Fields: `version`, `namespace`, `epoch`, `flags`, `tag`, `object_root`, `nonce`, `ciphertext`, `padding`, optional `sender_pubkey`, optional `signature`.
- Encoded as CBOR (recommended) or canonical JSON.
- Optional signature covers canonical header + ciphertext hash.

**ShardV1 (network unit)**
- Fixed bucket sizes: 2/4/8/16/32/64 KiB (configurable by profile).
- Header includes `tag`, `object_root`, `k`, `n`, `index`, plus `epoch`/`namespace`.
- Payload is opaque random‑looking bytes.

**Tags**
- Public feed tag: `H("feed" || publisher_pubkey || namespace)` (stable).
- Rendezvous tag: `H("rv" || recipient_pubkey || epoch || namespace)` (rotating).

**Namespaces**
- `0..=31` reserved for protocol/system use (see `SPEC.md`).
- Apps should use `>=32` unless intentionally extending a reserved namespace.

See `SPEC.md` for normative details and sizes.

## Web‑of‑Trust (WoT) policy (summary)

VEIL’s WoT is **local resource allocation**, not global truth:

- **Tiers:** Trusted / Known / Unknown / Muted / Blocked.
- **Inputs:** explicit local follow/mute/block, bounded transitive endorsements, optional behavioral signals.
- **Use:** forwarding quotas, cache retention caps, and UI ranking.
- **Safety:** unknowns retain a small budget to avoid ossification.

Recommended v1 defaults:
- 2‑hop max, strong decay, ≥2 endorsements threshold.
- Forwarding quotas: 70% Trusted, 25% Known, 5% Unknown.
- Eviction: rarity‑biased first, then trust tier, then age.

## Transport model (summary)

**Transport adapters** are pluggable lanes that move opaque bytes:

- **send(peer, bytes)** — best‑effort delivery.
- **recv() -> (peer, bytes)** — inbound payloads + peer identity.
- **max payload hint** — optional to pick bucket sizes.

Key properties:
- Lossy delivery is expected; ordering is not required.
- Lanes are local policy; shards contain no lane metadata.
- Multiple lanes can be active simultaneously (fast + fallback).

Implemented lanes include QUIC, Tor SOCKS5, WebSocket, and BLE (btleplug backend).

## Repository layout (top‑level)

- `SPEC.md` — protocol/library spec draft (`ObjectV1`, `ShardV1`)
- `ROADMAP.md` — implementation phases and milestones
- `crates/veil-core` — core types, hashing, tag derivation
- `crates/veil-codec` — object/shard encoding + validation
- `crates/veil-crypto` — AEAD + signing interfaces
- `crates/veil-fec` — FEC profiles + sharding
- `crates/veil-node` — runtime, forwarding, cache, ACK handling
- `crates/veil-transport-*` — transport adapters (QUIC, Tor, WebSocket, BLE)
- `crates/veil-sim` — e2e, performance, stress, and memory tests
- `apps/android-node` — Android foreground service wrapping Rust node + Flutter UI
- `apps/veil-vps-node` — VPS edge forwarder + hot cache
- `apps/veil-desktop` — Electron + React example
- `apps/veil-flutter-example` — Flutter example app
- `packages/veil-sdk-js` — JS SDK
- `packages/veil-sdk-dart` — Dart/Flutter SDK (FRB bridge)

## Quick start

```bash
cargo test --workspace
```

Run a runtime facade example:

```bash
cargo run -p veil-sim --example runtime_facade
```

## Protocol‑level highlights (developer view)

- **ObjectV1** — encrypted payload + optional signature + padding
- **ShardV1** — fixed‑bucket shard with tag, object root, `k/n/index`
- **Tags** — public feed tags and rotating rendezvous tags
- **ACKs** — optional ack objects for delivery confirmation
- **Cache** — rarity‑biased eviction to keep scarce shards longer
- **WoT** — local trust tiers for forwarding and storage quotas

See `SPEC.md` for normative details and `docs/` for app‑level schemas and runbooks.

## SDKs and client‑native support

**JS (React / React Native / browser)**
- `packages/veil-sdk-js` provides tag derivation, lane adapters, cache stores, and a client runtime scaffold.
- WASM bindings live in `crates/veil-wasm` (optional). Use `pure-js` backend in React Native.

**Dart / Flutter**
- `packages/veil-sdk-dart` wraps Rust core via Flutter Rust Bridge and provides lanes + cache stores.

## Examples and demos

- VPS edge forwarder profile: `apps/veil-vps-node`
- Desktop app + relay: `apps/veil-desktop`, `apps/veil-desktop-relay`
- Flutter example: `apps/veil-flutter-example`

## Further reading

- `SPEC.md` — protocol definition
- `ROADMAP.md` — staged implementation plan
- `docs/runbooks/` — deployment profiles and VPS guidance
