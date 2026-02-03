# Codec Test Vectors

These fixtures lock deterministic `veil-codec` behavior for cross-implementation compatibility.

## Files
- `object_v1_cbor.hex` - canonical CBOR bytes (hex) for the `sample_object()` test case
- `shard_v1_cbor.len` - expected encoded CBOR byte length for `sample_shard()`
- `shard_v1_cbor.blake3hex` - BLAKE3 digest (hex) of encoded `sample_shard()` CBOR bytes

## Update workflow
1. Change codec/schema logic intentionally.
2. Run:
   - `cargo test -p veil-codec --test golden_vectors -- --nocapture`
3. Copy the suggested replacement values from failing assertions into these files.
4. Re-run:
   - `cargo test --workspace`
   - `cargo clippy --workspace --all-targets -- -D warnings`

## Notes
- Treat vector changes as protocol-significant; mention them in PR descriptions.
- Keep fixtures ASCII, lowercase hex, and newline-terminated.
