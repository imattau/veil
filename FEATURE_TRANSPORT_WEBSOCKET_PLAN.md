# Feature Plan: WebSocket Transport Adapter

## Goal
Add a real network transport implementation that satisfies VEIL's byte-oriented `TransportAdapter` contract.

## Tasks

- [x] **Create dedicated crate**
  - Add `crates/veil-transport-websocket` to workspace.
- [x] **Implement adapter**
  - `WebSocketAdapter` with `send`/`recv` over a single websocket connection.
  - reconnect with exponential backoff.
  - bounded outbound/inbound buffering.
  - optional payload size hint enforcement.
- [x] **Graceful lifecycle**
  - worker thread + shutdown signal + clean join on drop.
- [x] **Test round-trip behavior**
  - local websocket echo test verifies adapter send/recv path.

## Result
VEIL now ships a concrete transport implementation suitable for early deployment and integration testing outside in-memory simulation.
