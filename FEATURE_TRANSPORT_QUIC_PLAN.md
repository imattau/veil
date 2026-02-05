# Feature Plan: QUIC Fast-Lane Transport

## Goal
Add a real fast-lane transport implementation with low-latency connectivity semantics.

## Tasks

- [x] **Create dedicated crate**
  - Add `crates/veil-transport-quic` to workspace.
- [x] **Implement adapter**
  - `QuicAdapter` implementing `TransportAdapter`.
  - outbound sends via QUIC uni streams.
  - inbound receive path from accepted QUIC uni streams.
  - queued send path with timeout controls.
- [x] **Identity/config support**
  - self-signed identity generator for local/dev use.
  - trust-store based client config.
- [x] **Add tests**
  - round-trip adapter send/recv between two local endpoints.
  - invalid peer validation.

## Result
VEIL now has a native QUIC transport lane suitable for high-performance fast-path delivery.
