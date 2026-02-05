# WoT Completion Plan

Goal: move WoT from local-v1 prioritization to a complete, explainable, and portable system.

## Phase 1: Core scoring + explainability
- [x] Add deterministic `score_publisher(pubkey, now_step) -> f64`.
- [x] Add explainability payload with score components and override reasons.
- [x] Add boundary tests for score bounds and explanation consistency.

## Phase 2: Trust graph import/export
- [x] Add JSON export/import for local WoT state.
- [x] Add round-trip tests for trust graph persistence.
- [ ] Add file-path helpers for trust graph snapshots.

## Phase 3: Runtime quota scheduling
- [ ] Add tier-aware forwarding queue ordering.
- [ ] Reserve explicit unknown-budget floor under load.
- [ ] Add per-tier forwarding/drop metrics.

## Phase 4: Endorsement ingestion path
- [ ] Define endorsement object schema (non-normative).
- [ ] Verify and ingest endorsements from runtime objects.
- [ ] Add duplicate suppression + staleness pruning.

## Phase 5: Cache policy hardening
- [ ] Enforce per-tier caps with global coordination.
- [ ] Tune rarity/trust/age/request weights with scenario tests.

## Phase 6: SDK surface
- [ ] Expose WoT score/explanation and trust import/export in SDK-js.
- [ ] Add docs and integration examples for UI ranking.
