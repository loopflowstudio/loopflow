---
status: proposed
seq: 3
---

# 03: Combine PRs Reconciliation

Make **Combine PRs** a first-class lifecycle operation with coherent run history.

## Estimated implementation size

~400-900 LOC across combine orchestration, reconciliation persistence, and API projections. (Revised upward — 01+02 showed atomicity/retry paths add significant code, and combine has similar multi-step GitHub+DB coordination.)

## What changed after 01 + 02 shipped

- Queue advancement and merge dedupe already exist and are reliable entry points.
- Queue role is currently projected from run stack status + live PR cache; no historical queue-role log exists.
- Live PR sync can infer `superseded` when originals close, but it cannot explain *why* they closed.
- `QueueRole::Superseded` and `WaveRunStackStatus::Superseded` are already defined and projected from `LivePrState::Closed`. Phase 03 converts this inference to durable fact with combine event linkage.
- `QueueNextAction::CombinePrs` is already returned for superseded runs — the UI affordance exists before the backing operation.
- `QueueOps` trait is the right extension point for combine operations, but currently only has five methods (`ensure_branch_checked_out`, `mark_ready`, `mark_draft`, `rebase_onto_default`, `scratch_clean`). Phase 03 must add `close_pr` and `create_combined_pr` to the trait and implement them in `RealQueueOps`.
- Per-wave reconcile locks serialize combine+reconcile safely — no new locking needed.
- `QueueBlockReason` enum needs new variants (e.g., `CombinePending`) added to the Rust enum *before* storing them. `FromStr` is strict and will reject unknown values.

## What exists after this step

- Combine PRs is no longer a detached git-only operation.
- Original iteration runs are preserved and linked to a durable combine event.
- Queue projection excludes superseded items automatically.
- API/Concerto can explain exactly what happened after combine.

## Core problem being solved

Current combine behavior can leave runs looking closed/superseded via live state, but with no explicit relationship to the combined PR or actor intent.

This step adds lifecycle reconciliation so stack history remains trustworthy.

## Combine event model

Persist a combine event with:

- `wave_id`
- `combined_pr_number`
- `combined_branch`
- `combined_run_id` (if synthetic run used)
- `actor`
- `created_at`
- `reconcile_status` (`complete`, `partial`, `retry_needed`)

Persist per-run links:

- `collapsed_into_pr_number`
- `superseded_by_run_id`
- `superseded_at`

## Operation flow

1. Acquire per-wave reconcile lock (reuse existing `QUEUE_RECONCILE_LOCKS`).
2. Resolve eligible open queue runs in wave via `LivePrSnapshot`.
3. Build combined branch from intended deltas.
4. Open combined PR via new `QueueOps::create_combined_pr`.
5. Close superseded PRs via new `QueueOps::close_pr`.
6. Persist combine event + supersession links in one reconciliation write.
7. Re-run `reconcile_wave_queue` to project final roles (existing function handles `Superseded` inference).
8. Resync live PR state via `build_live_pr_snapshot`.

## Atomicity expectations

Prefer all-or-nothing semantics for reconciliation writes.

If combined PR is created but reconciliation write fails:

- mark combine event as partial
- retry reconciliation safely
- avoid duplicate combined PR creation

## API changes

Run and wave projections should include:

- `is_superseded`
- `superseded_by_pr`
- `superseded_by_run`
- `combined_pr`
- `combine_event_id`

Queue endpoints should ignore superseded items when determining next actionable run.

## Backward compatibility policy

No compatibility shim needed for old combine records. Historical runs without combine links remain unlabeled in v1.

## Error handling

### Branch synthesis failure

- no supersession writes
- originals remain active
- return clear remediation message

### Close-originals failure

- combined PR may remain valid
- mark original runs as `pending_supersede_cleanup`
- retry closure + state sync without creating duplicate combine events

### Reconciliation write failure

- persist retry task
- do not hide original queue items unless confirmed superseded

## Test plan

### Unit tests

- combine event persistence is idempotent for same combined PR
- combine event writes expected supersession links
- queue projection excludes superseded items

### Integration tests

- combine creates one active landing target
- originals close and show superseded status in API
- webhook/poll merge reconciliation remains idempotent after combine
- partial failure paths remain recoverable

### Regression tests

- no orphaned runs after combine
- open PR counts remain correct post-combine

## Open questions

- Do we need a synthetic `combined_run_id` in v1, or can event + per-run links fully explain combine history?
- Should `superseded_by_run_id` point to a synthetic run or remain nullable when combine is event-only?

## Resolved questions (from 01+02)

- **Where does combine hook into reconciliation?** Through the existing per-wave lock + `reconcile_wave_queue` call. No new locking infrastructure needed.
- **How does the UI know to suggest combine?** `QueueNextAction::CombinePrs` is already projected for superseded runs.
- **Does combine need its own live PR fetching?** No — reuse `build_live_pr_snapshot` from `live_pr.rs`.

## Try it

- Create three stacked PRs, run Combine PRs, then verify `/v0/wave_runs?wave_id=<id>&order=stack` shows one active landing target and explicit supersession links.
- Repeat the same combine webhook payload and verify event processing is idempotent.

## Observability

Track:

- combine attempts
- combine success/failure
- partial reconcile count
- supersede cleanup retries

## Non-goals

- Queue UI redesign (step 04).
- Review artifact migration (step 04).
- Alternative combine strategies beyond current collapse semantics.

## Done when

- Combine PRs leaves a coherent, queryable run story.
- No closed-original orphan problem remains in Runs tab projections.
