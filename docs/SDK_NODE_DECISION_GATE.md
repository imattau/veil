# SDK vs Node Decision Gate

This document defines the decision gate for whether VEIL should remain SDK-first
or move to a node-everywhere model (UI apps over a local node API).

## Goal

Choose the simplest architecture that still meets:
- p2p client-to-client connectivity
- identity-bound trust and transport pinning
- reliable multi-lane delivery on mobile
- offline/latency-tolerant operation

## Decision Gate (SDK-Complete)

Stay SDK-first if all of the following are true:

1. **P2P QUIC works on mobile**
   - Direct client-to-client QUIC connections are reliable on Android.
   - QUIC identity pinning is supported and tested.
   - Fallback to WS when QUIC fails works.

2. **Multi-lane behavior is deterministic**
   - Lane priority is explicit (QUIC primary, WS fallback).
   - Swarm messaging across multiple lanes is tested.
   - Lane health metrics exist to observe failure modes.

3. **Identity + discovery are complete**
   - App can create identity + contact bundle.
   - Clients can exchange contact bundles (QR/links).
   - Relay discovery inputs are persisted and recoverable.

4. **Persistence + background behavior are acceptable**
   - Cache is stable across app restarts.
   - Reconnect logic works after background/kill.
   - Publish queue drains on reconnect.

If any of the above are not met, evaluate node-everywhere.

## Node-Everywhere Trigger

Switch to a node-everywhere model if:
- Android background limitations prevent p2p lanes from staying connected.
- The SDK is forced to re-implement too much node logic in each app.
- Multi-lane routing/debugging requires node-grade observability.

## Next Milestones (SDK Path)

1. QUIC primary -> WS fallback test coverage (automated).
2. Swarm harness for QUIC relay + identities (automated).
3. Contact bundle exchange flow documented (QR + deep link).
4. Mobile background/reconnect behavior validated.

## Next Milestones (Node Path)

1. Define local node API surface (RPC + websocket).
2. Minimal embedded node runtime profile.
3. UI client layer binds to local node.
