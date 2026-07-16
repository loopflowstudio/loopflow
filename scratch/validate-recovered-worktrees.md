# Validate recovered worktrees before committing a successor (W2-251)

## Contract (directive v1)

Task recovery must refuse unsafe worktree and branch state **before** inserting a
successor or moving any durable ownership. Today a no-active-PR recovery can
commit the successor before later PR rotation rejects an unrelated branch.

Done when:
1. all branch/worktree/PR adoption preconditions are computed **first**;
2. refusal leaves predecessor, successor link, PR sequence, leases, and worktree
   untouched;
3. supported between-PR recovery selects a deterministic next branch;
4. tests cover unrelated branch, dirty worktree, missing branch, active PR, and
   crash boundaries.

## The bug

Two recovery entry points move durable ownership and only later let PR rotation
check the worktree:

- `ops::task::resume_task_async` — `reconcile_task_pr` (no-op when there is no
  active PR) → `reconcile_process_liveness` (reaps the dead lease, moves status)
  → `resume_session` → `session.launch` (reserves the successor body's lease).
- `project_session::runner::inspect_outcome` supervisor loop — for each task:
  `reconcile_task_pr` → `reconcile_process_liveness` (reaps lease) →
  `ensure_working_pr` (rotation, checks branch) → `relaunch_inactive_process`
  (inserts successor body).

So when there is no active PR and the worktree sits on an unrelated branch (or is
dirty, mid-rebase, or dangling off a deleted branch), the lease is reaped, the
status moves, and a successor body is launched/reserved **before**
`ensure_working_pr_with_authority` rejects at the branch/clean/collision gate
(`ops/task.rs:2099`-`2138`). Ownership has already moved.

## Fix

A single read-only precondition pass — `task_recovery_adoption(store, session)`
— computed before any mutation in both paths. It reads durable PR state plus the
worktree and either returns the adoption plan or refuses. On refusal nothing is
touched (it performs no writes).

Checks, in order:
1. **crash boundary** — `engine::git::intervention_state` detects
   `rebase-merge`/`rebase-apply`/`MERGE_HEAD`/`CHERRY_PICK_HEAD`/`REVERT_HEAD`/
   `BISECT_LOG`. Refuse.
2. **worktree on a branch** — `current_branch`; detached HEAD refuses.
3. **branch ref exists** — `ref_exists(refs/heads/<current>)`; a HEAD dangling
   off a deleted branch refuses (the "missing branch" case).
4. **PR-state adoption**:
   - active PR → worktree must be on the active PR's branch (dirty allowed —
     ongoing work). Else refuse (unrelated branch).
   - no active PR → between-PR recovery. Compute the deterministic next branch
     with the same logic `ensure_working_pr` uses (`deterministic_next_branch`,
     factored out so there is one implementation). Worktree must be on the
     settled branch **or** the next branch. Else refuse (unrelated branch).
5. **dirty between-PRs** — the runner's strict rotation cannot carry a dirty
   tree (`carry_dirty = false`), so a dirty between-PR worktree refuses; point
   the human at `lf pr next` to carry edits forward. Dirty is allowed when an
   active PR owns the worktree.

Wiring:
- `resume_task_async`: `task_recovery_adoption` first (read-only), then
  `reconcile_task_pr`, then a post-reconcile `refuse_dirty_between_prs` guard
  (covers the active→settled transition where reconcile observes a merge), then
  `reconcile_process_liveness`, then `resume_session`.
- `inspect_outcome` supervisor loop: `task_recovery_adoption` first per task; on
  refusal, `tracing::warn!` and `continue` (leave that task untouched, keep
  observing the rest) — previously one bad branch aborted the whole project
  observation via `?`. Then `reconcile_task_pr`, `refuse_dirty_between_prs`,
  `reconcile_process_liveness`, `ensure_working_pr`, `relaunch_inactive_process`.

`ensure_working_pr_with_authority` is refactored to call `deterministic_next_branch`
so the gate and the rotation agree on what "the next branch" is.

## Why refusal leaves everything untouched

`task_recovery_adoption` is read-only. In the no-active-PR case
`reconcile_task_pr` is a no-op (nothing to observe), so a pre-reconcile refusal
touches neither the lease, the status, the PR sequence, nor the worktree. In the
active-PR case the gate runs before `reconcile_task_pr` too, so refusing prevents
even the PR observation. The post-reconcile dirty guard runs after
`reconcile_task_pr` may have settled a genuinely-merged PR (observing reality,
not inserting a successor) but before `reconcile_process_liveness` (lease) and
`resume_session` (successor) — so no successor is inserted and no lease moves.

## Tests (`ops::task::tests`, reusing `rotation_task`)

- `recovery_refuses_an_unrelated_branch_before_moving_ownership` — settled PR,
  worktree on an unrelated branch; refusal; assert PR sequence, lease, and
  worktree branch unchanged (and that `reconcile_process_liveness` would have
  reaped the lease without the gate).
- `recovery_refuses_a_dirty_worktree_between_prs` — settled PR, worktree on the
  settled branch, dirty; refusal naming `lf pr next`.
- `recovery_refuses_a_missing_branch` — HEAD dangling off a deleted branch ref;
  refusal.
- `recovery_adopts_an_active_pr_branch_and_allows_ongoing_work` — active PR,
  worktree on the active branch, dirty; returns `Active`; positive case.
- `recovery_refuses_a_crash_boundary` — worktree mid-rebase; refusal naming
  `rebase`.
- `between_prs_recovery_selects_the_deterministic_next_branch` — settled PR with
  a `next_slug`; worktree on the computed next branch; returns `BetweenPrs` with
  that branch (accepts the partial rotation).

## Open question

The active-PR-merged-during-reconcile + dirty transition is closed by the
post-reconcile `refuse_dirty_between_prs` guard. Settling a genuinely-merged PR
is observation (the serial sequence gains no new PR), so "PR sequence untouched"
(no successor PR inserted) still holds.
