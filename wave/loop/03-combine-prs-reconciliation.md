---
status: proposed
seq: 3
---

# 03: Combine PRs Reconciliation

Make **Combine PRs** a first-class lifecycle operation with coherent run history.

## Estimated implementation size

~250-600 LOC across combine operation, run-model updates, and API projections.

## What exists after this step

- Combine PRs is no longer a detached git-only operation.
- Original iteration runs are preserved and linked to the combined PR.
- Queue projection excludes superseded items automatically.
- API/Concerto can explain exactly what happened after combine.

## Core problem being solved

Current combine behavior creates a combined PR and closes originals, but runs still point to stale/closed PRs with no explicit relationship.

This step adds lifecycle reconciliation so stack history remains trustworthy.

## Combine event model

Persist a combine event with:

- `wave_id`
- `combined_pr_number`
- `combined_branch`
- `combined_run_id` (if synthetic run used)
- `actor`
- `created_at`

Persist per-run links:

- `collapsed_into_pr_number`
- `superseded_by_run_id`
- `superseded_at`

## Operation flow

1. Resolve eligible open queue runs in wave.
2. Build combined branch from intended deltas.
3. Open combined PR.
4. Close superseded PRs.
5. Persist reconciliation links in one transaction boundary where possible.
6. Resync live PR state.
7. Recompute queue roles.

## Atomicity expectations

Prefer all-or-nothing semantics for model updates.

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

No compatibility shim needed for old combine records.

For existing historical combines (without links), backfill best-effort:

- infer by branch pattern + close timestamps
- mark inferred links explicitly (`reconciled_inferred = true`)

## Error handling

### Branch synthesis failure

- no supersession writes
- originals remain active
- return clear remediation message

### Close-originals failure

- combined PR may remain valid
- mark original runs as `pending_supersede_cleanup`
- background retry closure + state sync

### Reconciliation write failure

- persist retry task
- do not hide original queue items unless confirmed superseded

## Test plan

### Unit tests

- combine event writes expected links
- queue projection excludes superseded items

### Integration tests

- combine creates one active landing target
- originals close and show superseded status in API
- partial failure paths remain recoverable

### Regression tests

- no orphaned runs after combine
- open PR counts remain correct post-combine

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
