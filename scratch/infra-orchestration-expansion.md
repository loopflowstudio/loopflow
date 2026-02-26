# 03: Orchestration Expansion

## Problem

`lfd` can orchestrate loop/watch/cron today, but it still behaves like a polling scheduler first and an event router second. That creates three gaps:

1. **Slow reactions:** watch/cron rely on 30s polling ticks even when push signals already exist (`/hooks/git`, GitHub webhooks).
2. **Flow expressiveness ceiling:** flow execution is mostly linear + single-step fork fan-out; conditional behavior is not first-class.
3. **Low operator visibility under load:** we can see wave runs, but not *why* a run was queued, coalesced, dropped, or which branch decisions were taken.

Who benefits:
- Wave authors: faster trigger-to-run latency and richer orchestration semantics.
- Operators: diagnosable trigger/queue behavior.
- Infra maintainers: explicit contracts instead of hidden scheduler behavior.

Why now: Pass 1/2 stabilized boundaries and contracts. This is the first point where we can expand orchestration behavior without compounding fragility.

## Approach

Build one explicit orchestration spine: **Activation ingress → deterministic flow planning → observable dispatch**.

### 1) Unified activation ingress (push first, polling fallback)

Add a single trigger entrypoint used by *all* stimuli:

- New module: `lfd/triggers/activation.rs`
- New type: `ActivationEnvelope { wave_id, stimulus_id, source, reason, dedupe_key, payload, observed_at }`
- New source enum: `ActivationSource::{WatchPoll, WatchPush, CronPoll, ListenWaveCompleted, Manual}`

All trigger producers call `enqueue_activation(...)`:
- Watch poller (existing behavior, fallback/reconcile path)
- Cron poller (existing behavior)
- `/hooks/git` + GitHub `push` webhook (new push fast path for watch stimuli)
- Wave completion listener for `listen` stimuli (new; currently modeled but not actually executed)

Queue semantics:
- `pending_activations` remains the dispatch queue.
- Add `activation_events` ledger table for immutable trace/debug.
- Coalesce by `dedupe_key` for bursty stimuli (latest SHA wins for watch/listen).
- Enforce per-wave queue cap (`activation_queue_limit`, default 20) with explicit drop records.

### 2) Deterministic flow enrichment

Add explicit, replayable branching to the flow language.

#### Conditional nodes

Extend `FlowItem` with `when`:

```yaml
- when:
    predicate:
      stimulus_kind: watch
    then:
      - implement
    else:
      - review
```

Keep predicates intentionally narrow (no arbitrary expression engine):
- `stimulus_kind`
- `activation_source`
- `changed_paths_any_prefix`

#### Richer fork composition

Extend fork branches to support multi-step branch plans (not just single-step branches):

```yaml
- fork:
    branches:
      - id: infra
        steps: [reduce, gate]
      - id: ux
        steps: [polish, gate]
    aggregate: synthesize
```

Execution contract:
- Branch plans are expanded once at run start.
- Branch outcomes + aggregate policy are persisted.
- Replay uses persisted decisions, not fresh reevaluation.

Persistence for determinism:
- Add `wave_run_activation` snapshot payload (source + trigger metadata) to `wave_runs`.
- Add `wave_run_flow_decisions` table (run_id, node_path, selected_branch, decided_at).

### 3) Operational safeguards (observability + backpressure)

Observability surface:
- New WS events:
  - `wave_activation_queued`
  - `wave_activation_coalesced`
  - `wave_activation_dropped`
  - `flow_branch_selected`
- New HTTP endpoint: `GET /waves/:id/activations` (latest activation ledger entries)
- Structured logs include `wave_id`, `stimulus_id`, `source`, `dedupe_key`, `queue_depth`, `decision_path`.

Backpressure behavior:
- Dispatcher starts runs only through scheduler slots (existing slot discipline retained).
- Queue caps + coalescing prevent unbounded buildup.
- Pollers remain active as reconciliation loops to recover from missed push events.

Research patterns applied:
- **Controller pattern (Kubernetes-style):** event-driven fast path + periodic reconcile safety net.
- **Webhook ingestion pattern:** verify/authenticate, dedupe by delivery key, enqueue async work.
- **Deterministic workflow pattern (Temporal-style principle):** persist inputs and branch decisions so replay is exact.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Polling-only improvements (lower intervals, more polls) | Minimal code churn | Still latent under load, still opaque, and increases background churn/IO. Doesn't unlock listen/push orchestration. |
| Full external event bus (Kafka/NATS) | Strong scalability primitives | Overkill for loopflow’s local-first architecture; adds operational surface that violates infra compactness goals. |
| Add push triggers ad hoc in each poller | Fast to prototype | Duplicates dedupe/queue logic across modules and recreates fragility Pass 1/2 just removed. |

## Key decisions

- **Single ingress API for activations.** Every trigger path (poll or push) writes the same envelope and queue contract.
- **Constrained conditions, not general scripting.** Deterministic, testable, debuggable flow behavior over expressive-but-opaque DSL power.
- **Persist decisions, not just outcomes.** Replayability requires storing why a branch was chosen, not inferring later.
- **Push is the fast path; polling is the safety net.** We do not remove loop/watch/cron polling compatibility.
- **Success criteria from “wild success”:** users see <5s watch-trigger latency on push-enabled repos and can answer “why did this run start?” from one activation log.
- **Failure we are explicitly preventing:** event storms causing silent drops, and conditional branches becoming non-replayable because trigger context was not persisted.
- **Known infra risks acknowledged:** guard against **abstraction creep** by adding one ingress seam (not many trigger-specific frameworks), and avoid **over-decomposition** by keeping trigger producers thin and policy centralized.

## Scope

- In scope:
  - Unified activation ingress + activation ledger
  - Push watch/listen trigger paths with polling fallback
  - Flow `when` nodes with constrained predicates
  - Multi-step fork branch composition + persisted branch decisions
  - Activation/flow decision observability and queue backpressure controls

- Out of scope:
  - Replacing scheduler slot model
  - Arbitrary expression engine for flow conditions
  - New hosted/studio auth roadmap items
  - Broad Concerto UI redesign
  - Persistence backend redesign beyond additive tables/columns

## Done when

- Trigger contract:
  - Watch/listen waves can start from push events; polling still recovers missed events.
  - `cargo test -p loopflow triggers` passes with new push + fallback coverage.
- Flow contract:
  - `when` conditions and multi-step fork branches execute deterministically and replay from stored run context.
  - `cargo test -p loopflow flow` passes with branch-decision persistence tests.
- Ops contract:
  - Operators can inspect activation source/reason/queue decision via API + logs.
  - Backpressure behavior is explicit (coalesced/dropped records visible, no silent loss).
- End-to-end:
  - `tests/e2e/test_smoke.sh` passes with no regression to existing loop/watch/cron behavior.

This directly advances Infra wave goals:
- **“Invest in the prompt engine and flow system (the differentiators)”**
- **“Maintain architectural compactness as features grow”**
