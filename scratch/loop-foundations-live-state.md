---
status: in_progress
seq: 1
source: wave/loop/01-foundations-live-state.md
---

# Foundations + Live State

## Problem

Stacked wave runs currently rely on inferred lineage and frozen PR snapshots. That breaks when people merge, close, or re-draft PRs from GitHub or local CLI outside the daemon's happy path.

Who benefits:
- Conductors managing many stacked iterations
- Queue automation in step 02 (promotion + merge advancement)
- Concerto/API users who need reliable open counts and queue head state

Why now: step 02 cannot be deterministic until ancestry and current PR state are both authoritative.

## Approach

Build a two-layer model: immutable run snapshots for history + mutable live PR state for “now”.

1. **Make run lineage explicit at write time**
   - Set `parent_run_id`, `parent_pr_number`, `stack_position`, `stack_group_id`, `stack_status` when creating each main run.
   - Keep `lineage_inferred` only for historical backfill rows.
   - Add/standardize store helpers for oldest-first stack listing, next-unmerged lookup, and descendant traversal.

2. **Introduce authoritative live PR cache**
   - Upsert `(repo_id, pr_number)` into `live_pr_states` with `state`, draft bit, refs, and sync timestamps.
   - Sync from GitHub pull data using three triggers: periodic poll, wave-scoped refresh before projection, merge-triggered refresh hook (for step 02).
   - On sync failure, keep last known state and mark projection stale; never invent “open”.

3. **Project API from live truth, keep snapshot historical**
   - Wave DTO derives `open_pr_count`, `stack_count`, and `has_stale_pr_state` from live projection.
   - Run DTO includes lineage fields plus `live_pr_state`, `live_pr_is_draft`, and `pr_state_stale`.
   - `snapshot.pr` remains untouched as point-in-time run history.

4. **Backfill safely**
   - One-shot migration orders main runs by `(iteration, started_at, id)` per wave.
   - Link each run to previous run, set `stack_position`, and mark inferred lineage.
   - Never overwrite explicit lineage on rows already populated by newer code.

5. **Research-informed pattern choice**
   - Use the common "materialized remote state + stale bit" pattern (cache remote truth locally, expose freshness explicitly).
   - Avoid webhook-only dependency in v1; polling + on-demand refresh gives safer degraded behavior.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Keep branch-name/iteration inference only | Minimal schema work | Breaks on rebases/manual branch changes and cannot support deterministic queue progression |
| Fetch GitHub PR state on every API request (no table) | Freshest possible per call | High API fanout/rate-limit risk, slow list endpoints, no durable last-known fallback |
| Webhook-only PR state updates | Lower polling overhead | Missed events during downtime become silent correctness bugs without replay infra |

## Key decisions

- Following wave principles directly: **"Keep stacked iterations and track ancestry explicitly."** and **"Make GitHub live PR state authoritative for current status."**
- Bold choice: stale/unknown is explicit API data (`pr_state_stale`, `unknown`), not hidden fallback behavior.
- Wild success target: merge a PR in GitHub UI and queue-facing endpoints reflect new truth within 60 seconds.
- Wild failure guard: if GitHub is unavailable, we degrade visibly (stale=true) instead of silently reporting incorrect open counts.
- Immutable history wins: run snapshots are never rewritten by live sync.

## Scope

- In scope:
  - explicit lineage metadata for main runs
  - best-effort historical backfill with inference marking
  - live PR state table and sync upserts
  - wave/run DTO projection from live state with stale markers
  - oldest-first stack ordering support for queue rendering
- Out of scope:
  - draft/ready promotion policy
  - merge-triggered rebase execution
  - combine PR reconciliation
  - Concerto queue UX polish

## Done when

- `cargo test --all` passes with coverage proving:
  - iteration N links to N-1 and increments `stack_position`
  - backfill does not overwrite explicit lineage
  - merged/closed/unknown live PR transitions project correctly
  - API `open_pr_count` is live-state derived, not snapshot-derived
- `GET /waves/{id}` reports accurate `open_pr_count`, `stack_count`, and `has_stale_pr_state` for a stacked wave.
- `GET /wave_runs?wave_id=<id>&order=stack` returns stable oldest-first lineage with correct per-run stale flags.
