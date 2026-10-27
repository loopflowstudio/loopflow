# LOO-626 plan and questions

## Done on this branch

- ci-fix admission no longer checks live work in the checkout; `OpsError::CheckoutBusy` and its held-landing path are deleted. `waited_task_landing_repairs_beside_the_open_task_session` (land_tests) proves a repair runs while the Task's Session stays open.
- Checkout restoration no longer refuses on running work.
- Locks kept, each with a one-line purpose: sync fence (`SyncOperation`), PR head mutation (`TaskPrMutationGuard`), landing observer (`lock_landing`), release (`ReleaseLock`), worktree lease, persistent-worktree placement, journal append, machine credentials, Home rotation (`disk.rs`), Claude config (`ConfigLock`), checkout admission (`store/sqlite/admission.rs`: 2 s SQLite membership serialization, not a write gate).
- Removal safety, not write gates, so kept: `cleanup_completed_task`, `require_idle`, `cleanup_landed_pr` (`checkout_execution_blockers`, now used only there).

## Remaining: release the fence when a sync is stuck

Jack Heart, 2026-10-26 (accepted): "we should clear the lock when we realize sync is stuck or errored or whatever." The fence is held only while a sync actually runs.

- Stops and errors: done. The fence is the sync process's flock; it drops on conflict return, recovery failure and exit (`a_running_sync_fences_agents_and_a_stopped_one_admits_them`).
- Stuck: not done. `resolve_sync_conflict` → `recover_sync` holds the fence for the whole `sync-conflicts` agent run. A live but wedged agent holds it indefinitely; no stall detection exists in lf today.
- Open choice (needs Jack): which signal means "stuck" — (a) the recovery agent's Session turns available without finishing the merge, (b) a wall-clock limit on the recovery agent, or (c) drop the fence before launching the agent at all, since a conflict-stopped merge already admits other writers. (c) is the smallest change and matches the stopped-merge semantics; it gives up fencing other agents out of the half-resolved merge while the resolver works.

## Notes

- `lock_task_pr_mutation` stays: it serializes a head move with clearing a head-pinned merge request, so Auto-merge never merges an unreviewed head. Held for the push/settlement only; contention refuses rather than waits.

Check: not rerun; branch tests from the implement pass apply unchanged (no code change in this pass).
