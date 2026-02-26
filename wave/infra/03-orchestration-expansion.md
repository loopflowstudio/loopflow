# 03: Orchestration Expansion

Flow-language enrichment: conditional nodes, richer fork composition, deterministic persistence.

Builds on unified activation ingress (shipped) — all trigger types now enqueue through `triggers/activation.rs` with explicit coalescing, audit logging, and observability.

## Scope

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

## Open questions

- Should `activation_queue_limit` be per-wave configurable (currently fixed default `20`)?
- Event-driven dispatch wakeups for lower latency under sustained load (current dispatcher interval: 1s)?

## Done when

- `when` predicates and multi-step fork branches execute deterministically from persisted run context.
- Replay does not reevaluate branch decisions.
