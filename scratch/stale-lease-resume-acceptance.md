# Stale-lease resume acceptance

## Problem

`ops::task::reconcile_process_liveness` bailed at the top whenever
`!session.status.is_process_active()`. A Task that is **Waiting** (W2-135) or
**Failed** (W2-122) is not process-active, so a dead lease it still carries
(`Legacy` / `Reserved` / `Active` from a body that vanished without a terminal
outcome) was never revoked. The explicit resume that follows then cannot reserve
a fresh body — the store's fencing WHERE-clause still sees a live lease.

## Fix

Reorder `reconcile_process_liveness` so the dead-lease revocation runs regardless
of status:

1. fresh-reservation guard (unchanged)
2. tmux liveness probe (unchanged) — a genuinely alive body is left alone
3. revoke + reap a dead `Legacy/Reserved/Active` lease **before** any status gate
4. only *then* early-return for non-process-active status; a Waiting/Failed
   Session keeps its status, and the resume relaunches against the reaped lease.

Process-active sessions are unaffected — same order, the added early-return sits
where the old terminal transition already began.

## Coverage

- `resume_revokes_a_dead_legacy_lease_on_a_waiting_task` (W2-135)
- `resume_revokes_a_dead_active_lease_on_a_failed_task` (W2-122)

Both assert the lease lands `Finished` (in memory and persisted) while status is
preserved.

## Deferred

The Project mirror `ops::project::reconcile_project_liveness` has the identical
top guard. The two urgent acceptance cases are Task-only, so this PR stays Task-
scoped; the Project symmetry is a follow-up.
