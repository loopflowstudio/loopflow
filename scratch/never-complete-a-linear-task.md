# W2-237 — Never complete a Linear Task before its PR and review gates

## Directive (v1, pending incorporation)

> Use W2-151 and W2-138 as live reproductions: both merged/auto-completed while
> required review or full contract remained open. Prevent Linear Task completion
> until every PR sequence is settled and required Human/Project review has an
> explicit approved outcome; prove merge races and serial continuation.

Steering commands incorporated: `cc_0226b894…` (codex resume, accepted but never
run — provider token revoked) and `cc_17e76bdd…` (this GLM-5.2 resume). Both say
the same thing: incorporate the directive exactly, do not weaken the gate, do not
silently complete the contract.

## Incident

W2-151 was marked `done` in Linear while its durable Task Session was still at a
required Human review (`ir_987b16aa…`) and PR #979 was still open. Completion
writeback outran the merge + review gates. W2-138 is the sibling (merged /
auto-completed with required review or full contract still open).

## Root cause

Two code paths set a Task Session to `Completed` and fire the `CompleteTask`
Linear writeback without checking that every PR is settled and every required
review is approved:

1. `ops::task::task_complete` (`lf task complete`) — allows completion with an
   *unpublished working PR* ("skipped PR"), and never checks required reviews.
2. `ops::task::reconcile_task_pr_with_authority` (merged + `AfterMerge::CompleteTask`)
   — on an observed (possibly out-of-band) merge it completes and writes back
   immediately, ignoring any pending required Human/Project review. This is the
   W2-151 / W2-138 race: the merge is observed before the gate review approves.

The Linear-side repair is also missing: once Linear says `done`, nothing reopens
it when the gates are actually still open, so the PM row, Session, PR, and review
state diverge instead of converging monotonically.

## Design

### One completion gate

A single predicate over store state, used by every completion path:

```rust
struct CompletionGate {
    satisfied: bool,
    blockers: Vec<String>, // actionable, human-readable
}
async fn task_completion_gate(store, session) -> CompletionGate
```

- **PRs settled:** `active_task_pr(session) == None` (every PR merged or
  explicitly abandoned). An open or unpublished working PR is a blocker.
- **Required reviews approved:** every `interaction_reviews` row for the task
  with `policy == Require` has `status == Completed && disposition == Approved`.
  A requested/active review, or one closed with `ChangesRequested`, is a blocker.

`defer`-policy reviews never block (they are non-required). Clean work with no
PR and no required reviews satisfies the gate vacuously, so `lf task complete`
for investigation/no-PR work still works.

### Wire the gate into both completion paths

- `task_complete`: compute the gate first. Unsatisfied → `Err` listing blockers
  (no status change, no writeback). The old "skipped working PR" auto-delete is
  removed — a working PR must be published+merged or explicitly abandoned.
- `reconcile_task_pr_with_authority` (merged + `CompleteTask`): compute the gate.
  Unsatisfied → set `Waiting` with an actionable reason naming the open review /
  PR, **do not** complete or writeback; still settle the merged PR. Satisfied →
  complete + writeback as today.

### Reconcile advances once the gate closes

`task_status` (and the runner-shared reconcile) re-runs the gate after PR
reconcile. If the task is not Completed, the merged `CompleteTask` PR is settled,
and the gate is now satisfied (e.g. the required review was approved after the
merge) → complete + writeback. This closes "merge arrives, review approves
later" without a second provider turn.

### Repair premature completion

On reconcile, if `status == Completed` but the gate is **not** satisfied
(legacy W2-151/W2-138 rows, or a review/PR regressed) → repair:

- revert the Session to `Waiting` with an actionable reason;
- preserve every row (Session, PRs, directives, review transcript — no deletes);
- queue a `ReopenTask` writeback (new `PmWritebackOperation::ReopenTask`) so
  Linear is reopened to its active workflow state and the PM row reconverges.

`PmWritebackState::Pending { operation: ReopenTask }` retries exactly like
`CompleteTask` until Linear confirms; idempotent (reopening an already-open
issue is a no-op at the Linear layer via state check).

### Linear reopen

`LinearClient::reopen_item(item_id)` resolves the team's active (`unstarted`)
workflow state and issues `issueUpdate { stateId }`. Mirrors `complete_item`.
The repair writeback calls it; the prevention gate makes it only ever fire on
genuinely premature completions.

### Idempotency / determinism

- The gate is a pure read over store state; running it twice changes nothing.
- Completion writeback fires at most once (Pending → Current on success); a
  duplicate reconcile against an already-Completed+Current session is a no-op.
- Reopen writeback fires at most once per repair; `retry_pm_writeback` handles
  both operations.
- Out-of-band merge, delayed review, writeback retry, restart, and duplicate
  observation all funnel through the same `reconcile_task_pr_with_authority` →
  gate → completion/reopen path, so they share idempotency.

## Tests

Integration test in `tests/pr_tests.rs` reproducing the W2-151 ordering:

1. Register a Task with a `CompleteTask` PR; insert a pending required Human
   review. `gh pr list` reports the PR MERGED. `task_status` → Session stays
   **not** Completed, status `Waiting` naming the open review, no writeback.
2. Approve the review (complete with `Approved`). `task_status` → Session
   `Completed`, writeback attempted once.
3. `task_status` again (duplicate) → still `Completed`, writeback still once.

Plus unit tests for:
- `task_completion_gate` satisfied/blocked across PR phases and review states.
- `task_complete` rejecting on an open PR / pending review / working PR.
- repair: a Completed session with a re-opened required review → reverted to
  `Waiting` + `ReopenTask` pending, all rows preserved.

## Done-when mapping

- ✅ PM completion writeback impossible until every PR merged/abandoned + every
  required review approved — gate on both completion paths.
- ✅ Waiting-on-review / open-PR Task stays open in Linear with an actionable
  reason — gate withholds completion + writeback; reason names the blocker.
- ✅ Reconciliation detects + repairs premature completion without losing
  Session/PR/directives/review transcript — repair reverts + queues reopen.
- ✅ Out-of-band merge, delayed review, writeback retry, restart, duplicate
  observation deterministic + idempotent — single gate + idempotent writebacks.
- ✅ Integration test reproduces W2-151 ordering, proves Linear cannot close
  early — `tests/pr_tests.rs`.
- ◑ Dogfood W2-151 through review and merge, verify final PM completion happens
  exactly once — requires live Linear + a human review; the code path is the
  one this PR hardens, but the live dogfood is a human run, noted in
  `scratch/questions.md`.

## Not included

- No change to the lifecycle gate *opening* of reviews (the runner's phase
  waitpoints). This PR makes the **completion** paths respect reviews that
  already exist; it does not alter when reviews are requested.
- No change to `defer`-policy reviews.
