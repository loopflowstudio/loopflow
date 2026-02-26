# 03: Orchestration Expansion

Expand trigger and flow capabilities after core boundaries and contracts are stable.

## Why this phase exists

Push responsiveness and richer orchestration are high leverage, but they should land on stable seams.

With boundary cleanup + contract hardening complete, this phase can expand behavior without compounding fragility.

## Status snapshot (2026-02-25)

### Milestone A (shipped)

1. **Unified activation ingress**
   - Added `triggers/activation.rs` as the shared trigger entrypoint.
   - Watch/cron pollers, loop ticker, manual run, listen completion, and push hooks now enqueue through one activation path.
2. **Push + listen activation paths**
   - Added `POST /hooks/git` and `POST /v0/hooks/github` push ingestion.
   - Listen stimuli now enqueue target-wave activations when source waves complete.
3. **Queue semantics + observability**
   - Activation queue now has explicit coalescing and drop records.
   - Added activation audit log + run linkage (`wave_runs.activation_log_id`).
   - Added WS activation events and `GET /v0/waves/{wave_id}/activations`.

### Milestone B (not shipped yet)

- Flow-language enrichment remains open:
  - `when` predicates
  - multi-step fork branch plans
  - persisted flow-branch decisions for deterministic replay

## Remaining scope (Milestone B)

### In scope

1. **Conditional flow nodes**
   - Add `when` node support with constrained predicates:
     - `stimulus_kind`
     - `activation_source`
     - `changed_paths_any_prefix`
2. **Richer fork composition**
   - Allow fork branches to run multi-step plans (`steps: [...]`) instead of single-step only.
3. **Deterministic persistence**
   - Persist run activation snapshot payload in `wave_runs`.
   - Add `wave_run_flow_decisions` (run_id, node_path, selected_branch, decided_at).
4. **Decision observability**
   - Emit `flow_branch_selected` WS event.
   - Include decision path/source in logs and replay diagnostics.

### Out of scope

- Replacing scheduler slot model
- Arbitrary expression engine for flow conditions
- Studio auth/hosting items from other waves
- Broad Concerto UI redesign

## Contract

- Existing loop/watch/cron behavior remains supported.
- Push fast-paths remain additive; polling remains reconcile safety net.
- Flow branch outcomes must be deterministic and replayable from persisted run context.

## Follow-ups after Milestone A

- Consider making `activation_queue_limit` per-wave configurable (currently fixed default `20`).
- Consider event-driven dispatch wakeups for lower latency under sustained load (current dispatcher interval: 1s).

## Validation

- `cargo fmt --all -- --check`
- `cargo clippy -p loopflow --all-targets -- -D warnings`
- `cargo test -p loopflow triggers`
- `cargo test -p loopflow flow`
- `tests/e2e/test_smoke.sh`

## Done when

- Watch/listen waves can start from push events and polling still recovers missed events.
- Operators can inspect activation source/reason/queue outcomes via API + logs.
- `when` predicates and multi-step fork branches execute deterministically from persisted run context.
- Replay does not reevaluate branch decisions.
