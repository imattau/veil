# Feature Plan: Tor SOCKS5 Fallback Transport

## Goal
Add a Tor-capable fallback transport that fits VEIL's transport-agnostic adapter contract.

## Tasks

- [x] **Create dedicated crate**
  - Add `crates/veil-transport-tor` to workspace.
- [x] **Implement adapter**
  - `TorSocksAdapter` with queued outbound sends through a SOCKS5 proxy.
  - parse peer as `host:port`.
  - enforce optional max payload hint.
  - expose outbound-only semantics (`recv()` disabled).
- [x] **Lifecycle and reliability behavior**
  - background worker thread with graceful shutdown.
  - connect/send timeouts on each outbound message.
- [x] **Test with local proxy mock**
  - verify SOCKS5 handshake path and payload delivery.
  - verify invalid peer validation.

## Result
VEIL now has a practical Tor-compatible fallback lane implementation suitable for censorship-resilient outbound delivery.
