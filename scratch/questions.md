# Open questions / assumptions — W2-145

## Migration ordinal 006 skipped (assumption, proceeded)

origin/main tops out at `0.11.005_provider_accounts`. The shared local store
also carries `0.11.006_context_launch_work` from another in-flight branch not
yet on main. To avoid a version-string collision that makes a store migrated by
one branch unreadable by the other (the shared-db hazard in wave memory), this
Task's migration is numbered **`0.11.007_task_session_successors`**, leaving a
deliberate gap at 006. `validate_set` requires only strictly-increasing
ordinals, not contiguity, so the gap is harmless. If `context_launch_work` never
lands, main will read 005 → 007.

## `lf rebase` / ledger blocked by shared-db incompatibility (worked around)

The installed `lf` (0.11.1) refuses to open `~/.lf/loopflow.db` because that
store already carries `0.11.006_context_launch_work`, unknown to this binary.
That blocked `lf rebase` (it errored before touching git) and makes `lf`'s
ledger-writing commands warn. Rebased onto origin/main with `git rebase`
directly (this worktree is solely owned — the rebase plan reported
`agent_launched: false`) and resolved the single `migrations.rs` conflict. Not a
code issue; it is the shared-store convention gap already tracked in wave memory.

## PR2 (task-actionability) not started

The `TaskActionHint { recover | resume | start_next_pr | complete | none }` on
the Now/Roadmap + status DTOs is the next serial PR, per the design doc. PR1
ships the model + `lf task recover` + proof.
