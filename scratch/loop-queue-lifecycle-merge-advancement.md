---
status: in_progress
seq: 2
source: wave/loop/02-queue-lifecycle-merge-advancement.md
---

# Queue Lifecycle + Merge Advancement

Adopt a deterministic landing queue with Draft-first semantics and lazy rebase.

## Estimated implementation size

~450-900 LOC across PR lifecycle orchestration, merge detection handlers, and queue reconciliation.

## What exists after this step

- New stack PRs open as Draft.
- Exactly one PR per wave is Ready at a time.
- Merging queue-head from any surface advances queue automatically.
- Only the next draft rebases to main (lazy rebase), not full descendants.
- Rebase conflict is explicit as a blocked queue state.

## Queue model

Represent queue role on each run projection:

- `ready` — next merge target
- `draft` — waiting in queue
- `blocked` — next item cannot advance until conflict resolved
- `merged` — landed
- `superseded` — replaced by Combine PRs or bypass reconciliation

Queue order is always chronological (`stack_position` ascending).

## Invariants

1. At most one `ready` open PR per wave.
2. If there is any open PR, oldest open non-blocked item should be `ready`.
3. All deeper open items must be `draft`.
4. A `blocked` queue-head prevents auto-promotion of deeper items.

## PR creation behavior

On run completion with PR creation enabled:

1. Create PR as Draft.
2. Sync live state.
3. Recompute queue roles.
4. If no existing `ready`, promote queue-head to Ready.

This keeps creation idempotent and consistent across retries.

## Promotion policy

### Auto-promotion candidate

- oldest unmerged item whose parent chain is merged/superseded
- not blocked
- live state is open

### Pre-promotion checks

- PR exists and is open
- scratch-clean gate passes
- no active merge-advancement lock for the wave

If checks pass, transition Draft -> Ready.

## Merge detection

### Supported paths

- GitHub webhook (`pull_request.closed` + `merged=true`)
- Polling fallback for local daemon setups

### Requirements

- Same advancement logic regardless of trigger source.
- Duplicate events must be idempotent.
- Event handler keyed by wave and PR number.

### Idempotency guard

Use wave-scoped lock or event dedupe table:

- `wave_id`
- `pr_number`
- `merged_at`
- `processed_at`

Skip reprocessing if already handled.

## Lazy rebase algorithm

When a `ready` PR merges:

1. Refresh live state for wave stack.
2. Mark merged item role `merged`.
3. Find next queue item with open draft PR.
4. Attempt rebase of that one branch onto `origin/main`.
5. Push with `--force-with-lease`.
6. Resync PR state.
7. Promote to Ready if rebase succeeded.
8. If conflict, keep Draft + mark `blocked` with metadata.

No full descendant cascade in v1.

## Conflict handling

Blocked state captures:

- failing run id / PR number
- conflict files summary (if available)
- rebase attempt timestamp
- last error message

Recovery actions:

- user resolves manually and pushes
- agent-assisted conflict fix (future)
- Combine PRs escape hatch

After recovery, rerun promotion reconciliation.

## Out-of-order merge behavior

Expected path is prevented by Draft state.

If bypass happens (admin action):

1. Sync live state.
2. Reconcile earlier queue items as superseded/merged equivalent.
3. Recompute queue-head.
4. Record audit event (`out_of_order_merge_detected`).

Do not silently keep invalid queue assumptions.

## Scratch cleanliness gate

Queue-head promotion requires no tracked `scratch/` diff.

Purpose:

- avoid scratch residue landing in main
- keep GitHub Land button usable without custom local flow

Failures remain Draft with actionable error.

## API additions

Expose queue status for UI and automation:

- `queue_role`
- `queue_block_reason`
- `queue_blocked_at`
- `next_action` (`open_pr`, `resolve_conflict`, `combine_prs`)

## Test plan

### Invariant tests

- multiple PR creations still produce single Ready item
- promotion respects chronological order

### Merge advancement tests

- merge queue-head -> exactly one next promotion
- duplicate merge events do not double-run rebase
- merge detection via poll and webhook produce identical state

### Conflict tests

- rebase conflict marks blocked and halts further promotion
- after manual fix and sync, blocked clears and Ready resumes

### Scratch gate tests

- scratch diff blocks promotion
- clean rerun succeeds without manual DB edits

## Observability

Add counters and logs:

- merge events received
- merge events processed
- rebase success/failure
- blocked queue count
- promotion latency after merge

## Non-goals in this step

- Full descendant rebase cascades.
- Custom GitHub status-check integration.
- Concerto visual redesign.
- Combine event modeling details.

## Done when

- The queue advances automatically after a merged Ready PR.
- Only one PR is mergeable at a time.
- Conflicts are explicit and recoverable, not silent drift.
