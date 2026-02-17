---
status: in_progress
seq: 2
source:
  - wave/loop/01-foundations-live-state.md
  - wave/loop/02-queue-lifecycle-merge-advancement.md
---

# Wave Queue + Live PR State (Consolidated)

## Goal

Make stacked wave lineage explicit, use live GitHub PR state as current truth, and enforce deterministic queue advancement (Draft -> Ready -> Merged) with exactly one Ready PR at a time.

## Current state on this branch

### Foundations completed

- Explicit run lineage is in place (`parent_run_id`, `parent_pr_number`, `stack_position`, `stack_group_id`, `stack_status`).
- Migration backfill preserves explicit lineage from newer code paths.
- Live PR state is authoritative for current run/wave projection behavior (`open_pr_count`, stale signaling).
- Guardrail test locks `WaveRunKind::Main` storage mapping (`main = 1`) to protect migration assumptions.

### Queue lifecycle completed

- Added queue orchestration (`lfd::queue`) with:
  - `reconcile_wave_queue`
  - idempotent `handle_pr_merged`
- Added durable queue persistence:
  - `wave_queue_blocks` (blocked reason/details)
  - `wave_pr_merge_events` (merge-event dedupe)
- Run completion now creates/updates PRs as Draft first, then reconciles.
- Merge advancement is unified across webhook and polling paths.
- Queue projection fields are exposed on run DTOs:
  - `queue_role`
  - `queue_block_reason`
  - `queue_blocked_at`
  - `next_action`
- Promotion follows lazy rebase semantics (immediate next eligible run only).

## Invariants

- Live PR cache is authoritative for current open/closed/merged state.
- Queue role is projected, not canonically stored.
- Block facts and merge dedupe are persisted.
- At most one Ready open PR per wave.
- Rebase/promotion stops on conflict and records actionable blocked state.

## Remaining validation work

- Verify runtime sync cadence/trigger behavior end-to-end (not only store/projection tests).
- Verify queue-facing consumers tolerate persistent stale-state flags when GitHub token is unavailable.
- Re-run full lifecycle coverage and confirm:
  - queue-head merge promotes exactly one next run
  - duplicate merge events remain idempotent
  - rebase conflict blocks and halts deeper promotion
  - clearing block + reconciliation resumes promotion
  - scratch-dirty gate reports `next_action=resolve_conflict`

## Risks to watch

- Any future enum/storage drift for `run_kind` can silently break lineage backfill assumptions.
- Stale-state signaling may appear noisy in no-token environments; clients must treat it as expected degraded mode.
- Queue role is not historical; immutable timeline views would need explicit event/audit modeling.

## Out of scope (v1)

- Full descendant rebase cascades
- GitHub merge-queue/status-check integration
- Canonical historical queue-role persistence

## Done when

- `cargo test --all` passes with lineage + queue lifecycle coverage.
- `GET /waves/{id}` reports accurate `open_pr_count`, `stack_count`, `has_stale_pr_state`.
- `GET /wave_runs?wave_id=<id>&order=stack` returns stable oldest-first runs with queue fields and stale metadata.
- Merges detected from webhook and polling paths advance the queue identically within one reconciliation cycle.
