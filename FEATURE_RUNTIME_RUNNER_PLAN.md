# Feature Plan: Node Runtime Runner Orchestration

## Goal
Provide a production-friendly runtime loop in `veil-node` so integrators can run the node continuously without hand-rolling tick/sleep/error loops.

## Tasks

- [x] **Define loop config**
  - Add tunables for start step, tick interval, error backoff, and max consecutive errors.
- [x] **Define loop exits**
  - Add explicit exit reasons for completed budgets, cancellation, and error thresholds.
- [x] **Add reusable callbacks path**
  - Support callback-aware ticking in loop mode via `tick_with_callbacks_ref`.
- [x] **Implement runner methods**
  - `run_steps(...)` for bounded loops.
  - `run_until(...)` for cancellation-controlled loops.
- [x] **Cover with tests**
  - Validate fixed-step completion and cancellation behavior.

## Result
`NodeRuntime` now has built-in orchestration helpers suitable for daemon/service integration with minimal extra glue code.
