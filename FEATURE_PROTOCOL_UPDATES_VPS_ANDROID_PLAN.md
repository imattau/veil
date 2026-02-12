# Protocol Update Plan: VPS + Android Node

## Scope
- Enable and operationalize protocol efficiency updates in both `apps/veil-vps-node` and `apps/android-node`.
- Updates covered: systematic mode for public posts, bloom-filter peer exchange, replica-estimate probabilistic forwarding, and traffic-estimation test coverage.

## Task Breakdown
1. Runtime defaults and policy wiring
- Status: Complete
- VPS: enable `probabilistic_forwarding` and `bloom_exchange` defaults in runtime config initialization.
- Android: enable conservative defaults in `default_protocol_config`.
- Keep systematic mode for public posts through namespace-aware erasure mode selection.

2. Deployment configuration surface (env vars)
- Status: Complete
- VPS env knobs:
  - `VEIL_VPS_PROBABILISTIC_FORWARDING`
  - `VEIL_VPS_FORWARD_MIN_PROBABILITY`
  - `VEIL_VPS_FORWARD_REPLICA_DIVISOR`
  - `VEIL_VPS_BLOOM_EXCHANGE`
  - `VEIL_VPS_BLOOM_INTERVAL_STEPS`
  - `VEIL_VPS_BLOOM_FALSE_POSITIVE_RATE`
- Android env knobs:
  - `VEIL_NODE_PROBABILISTIC_FORWARDING`
  - `VEIL_NODE_FORWARD_MIN_PROBABILITY`
  - `VEIL_NODE_FORWARD_REPLICA_DIVISOR`
  - `VEIL_NODE_BLOOM_EXCHANGE`
  - `VEIL_NODE_BLOOM_INTERVAL_STEPS`
  - `VEIL_NODE_BLOOM_FALSE_POSITIVE_RATE`

3. Reconstruction correctness for mixed/systematic traffic
- Status: Complete
- Android protocol reconstruction now derives erasure mode from shard header wire metadata, with runtime config as fallback.

4. Unit tests for app-level behavior
- Status: Complete
- Added Android protocol tests for:
  - default efficiency-policy enablement
  - reconstruction mode selection from shard wire headers

5. Network-traffic estimation tests
- Status: Complete (core runtime)
- Added node tests estimating:
  - probabilistic forwarding reduction
  - bloom exchange overhead
  - normal social usage total traffic
  - normal social usage including `100` post reads

6. Rollout sequence
- Status: Pending (ops)
- Stage 1: Deploy VPS with default-on bloom/probabilistic forwarding.
- Stage 2: Deploy Android with same features and env overrides for tuning.
- Stage 3: Observe traffic/latency and tune:
  - min forwarding probability
  - replica divisor
  - bloom interval and false positive rate

7. Acceptance criteria
- Status: Pending (ops verification)
- Compilation: app targets build cleanly.
- Tests: protocol and traffic-estimation tests pass.
- Runtime: no reconstruction regressions on systematic public post flow.
- Observability: measurable reduction in forwarded shard volume for common shards.
