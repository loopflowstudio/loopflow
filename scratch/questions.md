# LOO-626 plan and questions

## Done on this branch

- ci-fix admission no longer checks live work in the checkout; `OpsError::CheckoutBusy` and its held-landing path are deleted. `waited_task_landing_repairs_beside_the_open_task_session` (land_tests) proves a repair runs while the Task's Session stays open.
- Checkout restoration no longer refuses on running work.
- Locks kept, each with a one-line purpose: sync fence (`SyncOperation`), PR head mutation (`TaskPrMutationGuard`), landing observer (`lock_landing`), release (`ReleaseLock`), worktree lease, persistent-worktree placement, journal append, machine credentials, Home rotation (`disk.rs`), Claude config (`ConfigLock`), checkout admission (`store/sqlite/admission.rs`: 2 s SQLite membership serialization, not a write gate).
- Removal safety, not write gates, so kept: `cleanup_completed_task`, `require_idle`, `cleanup_landed_pr` (`checkout_execution_blockers`, now used only there).

- Stuck syncs (Jack Heart, 2026-10-26, accepted: "we should clear the lock when we realize sync is stuck or errored or whatever"): `recover_sync` releases the `SyncOperation` fence before the `sync-conflicts` resolver launches and reclaims it only to verify and push. A wedged resolver fences no one; its `lf sync --continue` adopts the stopped record. The scoped `LF_GIT_OPERATION_ID` hand-off and `OperationAuthorization::Borrowed` are deleted. `a_resolving_sync_admits_other_agents_until_it_publishes` proves it; stops/errors stay covered by `a_running_sync_fences_agents_and_a_stopped_one_admits_them`.

## Notes

- `lock_task_pr_mutation` stays: it serializes a head move with clearing a head-pinned merge request, so Auto-merge never merges an unreviewed head. Held for the push/settlement only; contention refuses rather than waits.

Check: `cargo clippy --all-targets -- -D warnings` clean; lib `git_operation` 3/3, `sync_tests` 41/41, `land_tests` 30/30 pass.
