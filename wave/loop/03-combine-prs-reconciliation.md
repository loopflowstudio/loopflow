---
status: proposed
seq: 3
---

# 03: Combine PRs Reconciliation

Make **Combine PRs** a first-class lifecycle operation with coherent run history.

## Estimated implementation size

~300-700 LOC across combine orchestration, reconciliation persistence, and API projections.

## What changed after 01 + 02 shipped

- Queue advancement and merge dedupe already exist and are reliable entry points.
- Queue role is currently projected from run stack status + live PR cache; no historical queue-role log exists.
- Live PR sync can infer `superseded` when originals close, but it cannot explain *why* they closed.

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

1. Resolve eligible open queue runs in wave.
2. Build combined branch from intended deltas.
3. Open combined PR.
4. Close superseded PRs.
5. Persist combine event + supersession links in one reconciliation write.
6. Re-run queue reconciliation (`reconcile_wave_queue`) to project final roles.
7. Resync live PR state.

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
