# VEIL

![VEIL Header](veil_header.png)

VEIL is a transport-agnostic, shard-native overlay for censorship-resistant public feeds and privacy-preserving delivery.  
Instead of routing semantic messages, VEIL routes opaque erasure-coded shards across independent network lanes.

## Project aims

VEIL is designed to be practical first:

- **Resilient propagation:** keep public feeds available under packet loss, congestion, and partial censorship.
- **Privacy-preserving delivery:** use rotating rendezvous tags and opaque shard transport for private delivery patterns.
- **Transport independence:** run the same core protocol over multiple lanes (for example QUIC + Tor/WebRTC) without changing shard/object formats.
- **Fast deployability:** provide a usable implementation that does not require a full mixnet to operate effectively.
- **Policy-local operation:** apply trust/payment policies to local resource allocation (forwarding, caching), not protocol validity.

## Repository layout

- `SPEC.md` - protocol/library spec draft (`ObjectV1`, `ShardV1`)
- `ROADMAP.md` - implementation phases and milestones
- `crates/veil-core` - core types, hashing, tag derivation
- `crates/veil-codec` - object/shard encoding and validation
- `crates/veil-crypto` - AEAD and signing interfaces
- `crates/veil-fec` - profile selection and erasure-coding sharder
- `crates/veil-node` - runtime, forwarding, cache, ACK handling
- `crates/veil-transport` - transport adapter abstractions
- `crates/veil-sim` - e2e, performance, stress, and memory tests

## Quick start

```bash
cargo test --workspace
```

## Optional fuzzing

```bash
cargo install cargo-fuzz
cargo fuzz run codec_decode --manifest-path fuzz/Cargo.toml
cargo fuzz run node_runtime_ingest --manifest-path fuzz/Cargo.toml
```
