# Feature Plan: Real Transport Integration Profile

## Goal
Move from standalone transport crates to runnable runtime profiles and smoke-tested startup behavior.

## Tasks

- [x] **Wire transport crates into simulation package**
  - Add websocket and tor transport crates as `veil-sim` dependencies.
- [x] **Add runnable profile example**
  - `transport_multi_lane_runtime` example using:
    - fast lane: websocket adapter
    - fallback lane: tor socks adapter
    - node orchestration: `NodeRuntime::run_steps`
- [x] **Add integration smoke test**
  - Boot mocked websocket and SOCKS5 endpoints.
  - Validate runtime starts/stops and fallback payload egress works.
- [x] **Add operations runbook**
  - Document env config knobs and deployment guidance.

## Result
VEIL now provides an end-to-end profile path from transport adapters to runtime orchestration with test and runbook coverage.
