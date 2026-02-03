# VEIL

VEIL is a transport-agnostic, shard-native overlay for censorship-resistant public feeds and privacy-preserving delivery.

## Workspace layout

- `SPEC.md` - draft normative protocol/library spec (ObjectV1 + ShardV1)
- `ROADMAP.md` - phased implementation plan for a Rust-first library
- `crates/veil-core` - primitive types, hashes, tag derivation
- `crates/veil-codec` - Object/Shard schema and encoding primitives
- `crates/veil-crypto` - AEAD/signing traits
- `crates/veil-fec` - profile selection and sharder entry point
- `crates/veil-node` - node state, cache, forwarding primitives
- `crates/veil-transport` - transport lane abstraction
- `crates/veil-sim` - simulation scaffolding

## Quick start

```bash
cargo test --workspace
```

## Fuzzing (optional)

```bash
cargo install cargo-fuzz
cargo fuzz run codec_decode --manifest-path fuzz/Cargo.toml
cargo fuzz run node_runtime_ingest --manifest-path fuzz/Cargo.toml
```
