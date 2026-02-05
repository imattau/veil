# Feature Plan: Durable Node State Persistence

## Goal
Add a low-impact persistence layer so VEIL node state can be snapshotted and restored across restarts without changing protocol behavior.

## Tasks

- [x] **Define persistence scope**
  - Persist `NodeState` fields needed for continuity: subscriptions, cache metadata/bytes, replica estimates, reconstruction inbox, and pending ACK retries.
- [x] **Add serializable state model**
  - Ensure core node state structs are CBOR-serializable with `serde`.
- [x] **Implement persistence helpers**
  - Add `encode_state_cbor`, `decode_state_cbor`, `save_state_to_path`, `load_state_from_path`, `load_state_or_default`.
- [x] **Expose API from crate root**
  - Export persistence module through `veil-node`.
- [x] **Validate behavior with tests**
  - Round-trip state through bytes and files.
  - Confirm missing-file bootstrap returns default state.

## Result
`veil-node` now supports simple durable snapshots while keeping runtime and wire protocol unchanged.
