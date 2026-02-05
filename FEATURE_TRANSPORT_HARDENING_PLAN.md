# Feature Plan: Transport Hardening + Metrics Hooks

## Goal
Improve operational reliability by exposing lightweight per-adapter telemetry counters without changing protocol behavior.

## Tasks

- [x] Add metrics counters to websocket adapter (queue/send/recv/reconnect).
- [x] Add metrics counters to Tor SOCKS adapter (queue/send attempts/success/errors).
- [x] Add metrics counters to QUIC adapter (queue/send attempts/success/errors/recv drops).
- [x] Expose snapshot APIs on each adapter (`metrics_snapshot()`).
- [x] Extend adapter tests to assert metrics move as traffic flows.

## Result
Operators and runtime wrappers can now poll transport health directly and react to lane degradation with explicit local policy.
