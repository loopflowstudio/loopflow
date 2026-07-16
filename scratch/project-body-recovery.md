# W2-169 slice: Project-body recovery

Serial PR after #968 (stale-lease Task reconcile). Slug: `project-body-recovery`.

## Why this is next

#968 fixed `ops::task::reconcile_process_liveness`: a dead
`Legacy/Reserved/Active` lease on a **Task** was only revoked when the Session
was process-active, so a Waiting/Failed Task's explicit resume could not reserve
a fresh body. `ops::project::reconcile_project_liveness` (project.rs:551) carries
the **identical top guard** (`if !is_process_active { return Ok(()) }`) — a
Waiting/Failed/Blocked **Project** Session with a dead lease has the same defect.
This is the deferred Project mirror and the first concrete step of the contract's
"Project-body recovery and active-count semantics" slice.

## Fix (mirror of #968)

Reorder `reconcile_project_liveness`:

1. fresh-reservation guard (unchanged)
2. tmux liveness probe (unchanged) — an alive body is left alone
3. revoke + reap a dead `Legacy/Reserved/Active` lease **before** any status gate
4. only then early-return for non-process-active status; a Waiting/Failed/Blocked
   Project keeps its status, and the resume relaunches against the reaped lease.
   A process-active Project still transitions to `Failed` as before.

Process-active Projects are unaffected (same order; the added early-return sits
where the old `Failed` transition began).

## Coverage

Two regressions in `ops::project` tests, mirroring the Task pair:
- resume revokes a dead legacy lease on a Waiting Project
- resume revokes a dead active lease on a Failed Project

Both assert the lease lands `Finished` (memory + persisted) while status is
preserved, and process-active reconcile still fails the Session.

## Not in this PR (later serial slices, per contract)

- **Active-count semantics** (evidence #2: five active Projects, every body dead)
  — desired-intent vs observed-live-body counts as distinct typed fields; a dead
  body can never make a live-process count positive. Own slice; touches the
  reconciliation projection + CLI/Swift consumers.
- PR observation convergence (evidence #3, PR #903 open-after-merge).
- Side-effect-free inspection + explicit sync APIs (evidence #4, #5, `lf wt list`
  / `sync_main`).
- Shared status consumers + two-Wave restart dogfood.

W2-169 is NOT completed by this PR; bare `lf pr land --next <slug>` keeps the
Task open for the slices above.
