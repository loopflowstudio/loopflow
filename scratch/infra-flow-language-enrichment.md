# Flow Language Enrichment (Infra Pass 3, Milestone B)

Conditional flow nodes, multi-step fork branches, persisted flow decisions, and decision observability.

## Context

Pass 3 Milestone A shipped unified activation ingress — watch/cron/loop/manual/listen all route through `triggers/activation.rs` with explicit queue semantics and observability. Milestone B enriches the flow language itself.

## Scope

### 1. Conditional flow nodes

Add `when` node support with constrained predicates:

- `stimulus_kind` — match on activation trigger type
- `activation_source` — match on source wave or hook origin
- `changed_paths_any_prefix` — match on push payload file paths

Flow YAML gains a `when:` key on step nodes. Predicates are evaluated against the persisted activation snapshot payload.

### 2. Richer fork composition

Allow fork branches to run multi-step plans (`steps: [...]`) instead of single-step only. A fork branch becomes a sub-flow: ordered steps with the same direction/area context as the parent.

### 3. Deterministic persistence

- Persist run activation snapshot payload in `wave_runs` (activation context available for replay).
- Add `wave_run_flow_decisions` table: `run_id, node_path, selected_branch, decided_at`.
- Flow decisions are recorded at evaluation time and replayed on re-run without re-evaluation.

### 4. Decision observability

- Emit `flow_branch_selected` WebSocket event when a conditional or fork branch is chosen.
- Include decision path and source in logs and replay diagnostics.

## Out of scope

- Replacing scheduler slot model
- Arbitrary expression engine for flow conditions
- Studio auth/hosting items from other waves
- Broad Concerto UI redesign

## Contract

- Existing loop/watch/cron behavior remains supported.
- Push fast-paths remain additive; polling remains reconcile safety net.
- Flow branch outcomes must be deterministic and replayable from persisted run context.
- Replay does not re-evaluate branch decisions.

## Validation

- `cargo fmt --all -- --check`
- `cargo clippy -p loopflow --all-targets -- -D warnings`
- `cargo test -p loopflow triggers`
- `cargo test -p loopflow flow`
- `tests/e2e/test_smoke.sh`

## Done when

- `when` predicates and multi-step fork branches execute deterministically from persisted run context.
- Replay does not re-evaluate branch decisions.
- Decision events visible via WebSocket and API.
- Watch/listen waves can start from push events and polling still recovers missed events (preserved from Milestone A).
