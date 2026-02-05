# Feature Plan: Unified Transport Health Snapshot Trait

## Goal
Provide a uniform health surface across transport adapters so runtime policy and operators can inspect lane quality without transport-specific code.

## Tasks

- [x] Add `TransportHealthSnapshot` to `veil-transport::adapter`.
- [x] Extend `TransportAdapter` with `health_snapshot()` (default empty counters).
- [x] Implement concrete snapshots for:
  - in-memory adapters
  - websocket adapter
  - tor socks adapter
  - quic adapter
- [x] Add/extend tests to verify snapshot counter behavior.

## Result
Any runtime can query per-lane queue/send/receive/error/reconnect counters through a single trait-level API.
