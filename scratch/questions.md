# LOO-626 plan and questions

## Done on this branch

- ci-fix admission no longer checks live work in the checkout; `OpsError::CheckoutBusy` and its held-landing path are deleted. `waited_task_landing_repairs_beside_the_open_task_session` (land_tests) proves a repair runs while the Task's Session stays open.
- Checkout restoration no longer refuses on running work.
- Locks kept, each with a one-line purpose: sync fence (`SyncOperation`), PR head mutation (`TaskPrMutationGuard`), landing observer (`lock_landing`), release (`ReleaseLock`), worktree lease, persistent-worktree placement, journal append, machine credentials, Home rotation (`disk.rs`), Claude config (`ConfigLock`), checkout admission (`store/sqlite/admission.rs`: 2 s SQLite membership serialization, not a write gate).
- Removal safety, not write gates, so kept: `cleanup_completed_task`, `require_idle`, `cleanup_landed_pr` (`checkout_execution_blockers`, now used only there), and `git/build_cleanup.rs` `ownership` (refuses to delete build output under an unfinished or running Task's checkout).

- Stuck syncs (Jack Heart, 2026-10-26, accepted: "we should clear the lock when we realize sync is stuck or errored or whatever"): `recover_sync` releases the `SyncOperation` fence before the `sync-conflicts` resolver launches and reclaims it only to verify and push. A wedged resolver fences no one; its `lf sync --continue` adopts the stopped record. The scoped `LF_GIT_OPERATION_ID` hand-off and `OperationAuthorization::Borrowed` are deleted. `a_resolving_sync_admits_other_agents_until_it_publishes` proves it; stops/errors stay covered by `a_running_sync_fences_agents_and_a_stopped_one_admits_them`.

- Sync verification no longer checks tracked dirt (concept review, 2026-10-26): `verify_sync` and `verify_control_completion` dropped their checks, along with `SyncExpectation.tracked_dirty` and `tracked_dirty_state`. Postconditions cover only what the push publishes: no active merge, the expected branch, the pinned target and original head as ancestors, and no unmerged paths. `an_edit_beside_a_committed_resolution_leaves_the_sync_verified` and `continue_completes_beside_existing_tracked_edits` prove it. Known effect: a writer's `git add` during resolution can land in the resolver's merge commit.

## Sync fence scope (done 2026-10-26)

- `start_owned_sync` now refuses an existing merge, then fetches and plans, and only then takes the fence; `a_stalled_fetch_admits_other_agents` blocks upload-pack and proves `fence_agent_process` admits a launch (fails on the old order). The `SyncOperation` doc now says "held from the merge through publication".
- A commit landing during the unfenced fetch can't be reset away: `SyncPlan.head` records the commit the plan classified, and `revalidate_start` refuses under the fence when HEAD moved since (this also closes the same gap `sync_with_plan` already had). `a_commit_after_planning_is_never_reset_away` proves it.
- Hung sync (Jack Heart, 2026-10-26, operator reading of "imagine Mitchell Hashimoto reviews this PR"): a live sync keeps the fence with no timeout. The refusal names the holder, its target and how long it has held the lock, and says to kill the pid to continue (`owner_label`, `GitOperationOwner.locked_at`). The push passes `http.lowSpeedLimit=1`/`http.lowSpeedTime=300`. A `git add` landing in the resolver's merge commit is accepted and documented in `docs/architecture/delivery.md`.

## Notes

- `lock_task_pr_mutation` stays: it serializes a head move with clearing a head-pinned merge request, so Auto-merge never merges an unreviewed head. Held for the push/settlement only; contention refuses rather than waits.

Gate (2026-10-26): `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` clean; `ops::git_operation` 4/4; integration `sync_tests land_tests ops_ ci_fix` 76/76 (386 s, git-heavy fixtures). `a_stalled_fetch_admits_other_agents` now stops waiting if the sync finishes without fetching.

Gate 2 (2026-10-26, HEAD d6ca976 + stale-reference sweep): `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` clean; `ops::git_operation` 4/4; `sync_tests` 43/43. Swept `LF_GIT_OPERATION_ID` from the Swift GUI env scrub and `docs/architecture-reference.md`; `tests/e2e/test_sync_safety.sh` now asserts the new contract (a held resolver admits an agent; a second sync refuses the existing merge). Swift build deferred to CI (one string literal removed).
- Unresolved, not this branch: `tests/e2e/test_sync_safety.sh` cannot run on main either — its `opencode` stand-in prints text, but the harness now runs `opencode serve --stdio` and parses a JSON endpoint ("expected value at line 1 column 1" at the first provider launch). CI does not run the script. Needs a Task to give the stand-in a server endpoint or move the scenarios to Rust integration tests.
