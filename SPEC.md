# VEIL Protocol and Library Specification (Draft v0.1)

Status: Draft  
Target: VEIL Rust workspace (`crates/*`)

## 1. Scope

VEIL is a transport-agnostic, shard-native overlay for censorship-resistant public feeds and privacy-preserving delivery.

This draft defines normative behavior for:
- Object and shard schemas
- Tag derivation
- Object build/encrypt/shard flow
- Subscription-based forwarding
- Reconstruction and ACK behavior
- Rarity-biased cache policy
- Node-Everywhere operational model

Keywords **MUST**, **SHOULD**, **MAY** are interpreted as in RFC 2119.

## 1.1 Node-Everywhere Model (Normative)

VEIL treats all participants as functionally equivalent nodes. The UI is a thin client over a local node.

**Functional symmetry**
- All participants (mobile P2P clients and VPS nodes) MUST implement the same shard forwarding, caching, and reconstruction logic.
- Nodes MUST treat incoming data as opaque, fixed-size shards; forwarding decisions MUST be based on local policy and subscriptions, not on payload content or origin.
- Shard and object formats MUST be identical across all transport lanes (e.g., QUIC, WebSocket, Tor, WebRTC).

**Architectural decoupling**
- The node process MUST own networking, erasure coding, shard cache, and WoT policy.
- The UI layer MUST NOT implement protocol logic; it communicates via a stable local API (RPC/WS).
- Local policy MUST be sovereign: trust tiers and quotas are determined by local state only.

**Operational resilience**
- On mobile platforms, the node SHOULD be able to run as a background or foreground service to keep lanes alive and drain queues.
- CPU-intensive crypto and reconstruction SHOULD be isolated from the UI process.
- Nodes MUST buffer outbound payloads and drain queues after connectivity loss.

**Identity and discovery**
- Node identity, pinning, and contact bundles MUST be managed by the node.
- Applications SHOULD use namespaces `>= 32` to avoid collisions across UI flavors.

## 2. Primitives

- **Object**: application-level encrypted unit that is later split into shards.
- **Shard**: fixed-size network unit used for forwarding, dedupe, and cache.
- **Tag**: 32-byte opaque subscription identifier.

### 2.1 Reserved Namespaces

Namespaces are `u16`. The range `0..=31` is reserved for protocol/system use.
Applications SHOULD use values outside the reserved range unless explicitly
extending a reserved namespace definition.

Recommended assignments:
- `0` — system/protocol coordination
- `1` — public social feed (default)
- `2` — private messaging / vault
- `3` — WoT / endorsements
- `4` — relay/bootstrap coordination
- `5` — app-level bundles/schemas

## 3. Tag Derivation

Implementations MUST produce 32-byte tags using a cryptographic hash function (`H`, recommended BLAKE3).

- `feed_tag = H("feed" || publisher_pubkey || u16(namespace))`
- `rv_tag = H("rv" || recipient_pubkey || u32(epoch) || u16(namespace))`

`epoch` MUST be an integer window. Practical mode default: `EPOCH_SECONDS = 86400`.
Implementations MAY support an overlap transition window where both current and
next epoch rendezvous tags are accepted/forwarded near epoch boundaries to
reduce synchronized rotation fingerprints and tolerate clock skew.

## 4. ObjectV1 Schema

ObjectV1 MUST contain:
- `version: u16 = 1`
- `namespace: u16`
- `epoch: u32`
- `flags: u16`
- `tag: bytes32`
- `object_root: bytes32` (or derivable from payload)
- `nonce: bytes24`
- `ciphertext: bytes`
- `padding: bytes`

ObjectV1 MAY contain:
- `sender_pubkey: bytes32`
- `signature: bytes64`

Flags bit assignments:
- `0x0001` signed
- `0x0002` public
- `0x0004` ack_requested
- `0x0008` batched

Encoding SHOULD use CBOR with deterministic/canonical options.

## 5. ShardV1 Schema

ShardV1 MUST contain:
- `version: u16 = 2`
- `namespace: u16`
- `epoch: u32`
- `tag: bytes32`
- `object_root: bytes32`
- `profile_id: u16`
- `erasure_mode: u8` (`0=systematic`, `1=hardened_non_systematic`)
- `bucket_size: u32`
- `k: u16`
- `n: u16`
- `index: u16` (`0..n-1`)
- `payload: bytes[bucket - header_len]`

Allowed bucket sizes are `2 KiB`, `4 KiB`, `8 KiB`, `16 KiB`, `32 KiB`, `64 KiB`.
Implementations MAY add optional upward bucket jitter (choosing a larger
fitting bucket) to reduce size-correlation leakage.

`shard_id` MUST be `H(shard_bytes)`; nodes MUST dedupe by `shard_id`.

## 6. Profiles and Limits

Default profiles:
- `PROFILE_MICRO`: `id=1`, `k=2`, `n=3`, buckets `[2 KiB, 4 KiB, 8 KiB]`
- `PROFILE_SMALL`: `id=2`, `k=6`, `n=10`, buckets `[16 KiB, 32 KiB]`
- `PROFILE_LARGE`: `id=3`, `k=10`, `n=16`, buckets `[32 KiB, 64 KiB]`

Defaults:
- `TARGET_BATCH_SIZE = 96 KiB`
- `MAX_OBJECT_SIZE = 256 KiB`
- `EPOCH_SECONDS = 86400`
- `CACHE_TTL = 90 min`

## 7. Object Pipeline

### 7.1 Batching
- Producers SHOULD batch queued items until `TARGET_BATCH_SIZE`.
- Producers MUST NOT exceed `MAX_OBJECT_SIZE`.
- Interactive mode MAY flush immediately with smaller bucket targets.

### 7.2 Encrypt and Sign
- Payload bytes MUST be encrypted using AEAD XChaCha20-Poly1305 (or profile-compatible equivalent).
- AEAD associated data MUST bind `tag || namespace || epoch`.
- If signing is enabled, signature MUST cover canonical object header and ciphertext hash.

### 7.3 Padding
- Implementations SHOULD pad object bytes to align with erasure/bucket grouping and reduce size leakage.
- Implementations MAY apply bounded bucket jitter (e.g., choose one of the next
  larger fitting buckets) as an obfuscation policy.

## 8. Erasure and Sharding

- Implementations MUST select `(k, n, bucket)` using object size profile rules.
- Reed-Solomon encoding MUST default to a hardened non-systematic profile where
  source blocks are deterministically transformed before RS encoding, so first
  `k` shards are no longer direct plaintext-ciphertext chunks.
- Implementations MAY offer a systematic compatibility mode for constrained or
  legacy environments.
- Namespace policy MAY require systematic mode (for example public feed
  namespace `1`) to optimize common-case receive cost.
- Any set of `k` unique shard indices MUST be sufficient for decode.

## 9. Delivery and Forwarding

### 9.1 Multi-lane policy (practical default)
- Lane A SHOULD send `k+2` unique shards to 2 peers.
- Lane B SHOULD send 2 unique fallback shards.
- If ACK timeout triggers, sender SHOULD escalate by sending additional unsent shards in small batches.

### 9.2 Subscription forwarding
- Nodes MUST drop duplicate `shard_id`.
- Nodes MUST forward only subscribed tags.
- Nodes MAY briefly cache unsubscribed-tag shards without forwarding.
- Nodes MAY apply replica-estimate probabilistic forwarding to reduce floods of
  high-replica shards, while keeping a non-zero minimum forwarding probability.

### 9.3 Namespace signature policy (optional hardening)
- Implementations MAY define namespaces that require signed objects.
- For those namespaces, objects that are unsigned or fail signature
  verification SHOULD be rejected and SHOULD NOT be promoted into long-lived
  shard cache state.

## 10. Reconstruction and ACK

- Receiver MUST group shards by `object_root`.
- Receiver MUST attempt decode once `>=k` unique indices are present.
- Receiver MUST verify and decrypt object before delivery.
- Receiver SHOULD send ACK on successful delivery; ACK MAY use compact profile (e.g., `k=2,n=3`).

## 11. Cache and Eviction

Node state MUST include:
- `subscriptions: set<tag>`
- `cache: map<shard_id, {shard_bytes, expiry, last_seen}>`
- `replica_estimate: map<shard_id, score>`

Under pressure:
- Expired entries MUST be evicted first.
- Remaining evictions SHOULD prefer removing most common shards first (rarity-biased retention).

### 11.1 Bloom-filter peer exchange (optional)
- Peers MAY exchange compact Bloom summaries of recently seen shard ids.
- A Bloom exchange payload MUST include `{version, epoch, filter}`.
- Receivers SHOULD use false-positive-tolerant set difference to request/send
  only shards likely missing on the remote peer.

## 12. Security and Privacy Notes

- Transport encryption (e.g., QUIC/TLS/WebRTC DTLS) is REQUIRED in deployment.
- Padding SHOULD be enabled by default to reduce traffic analysis by size.
- Implementations SHOULD rotate rendezvous tags by epoch and support overlap windows for clock skew.
- Signature verification policy SHOULD be namespace-specific.

## 13. Compatibility

- This document defines `ObjectV1(version=1)` and `ShardV1(version=2)`.
- Future versions MUST be additive where possible and negotiated by explicit version fields.
