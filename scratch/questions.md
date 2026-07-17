# Open questions / assumptions

## Rebase onto origin/main (95e453fc3)

Resolved conflicts in `ops/task.rs` and `lf/commands/waves.rs`. Decisions taken
headlessly:

1. **`committed_follow_up_range` returns an enum now.** Main (#1050/#1056)
   replaced the `Option` this branch was written against with
   `CommittedFollowUp { ProvenEmpty, Range, Unprovable }`. The design doc's
   `committed_carry.is_none()` became
   `!matches!(&committed_carry, CommittedFollowUp::Range { .. })`, preserving
   main's fail-closed treatment of `Unprovable`. The branch's contribution is
   unchanged: `&& !has_pending_directive(session)` — a pending directive
   independently authorizes the same serial successor.

2. **Two test env guards now coexist in `ops/task.rs`.** Main added
   `TaskLaunchEnv` (fake tmux on `PATH`, `LF_BIN`, store vars); this branch
   added `StoreEnvGuard` (store vars only, holds the journal env lock). I first
   collapsed mine into main's, but `TaskLaunchEnv`'s fake tmux reads as a live
   body to any concurrent liveness test, so store-only tests must not borrow it.
   Both are kept, with a doc comment on `StoreEnvGuard` naming the distinction.

3. **Pre-existing test race, fixed rather than routed around.** Three liveness
   tests (`resume_revokes_a_dead_legacy_lease_on_a_waiting_task`,
   `resume_revokes_a_dead_active_lease_on_a_failed_task`,
   `reconcile_process_liveness_consumes_queued_resume_before_settling`) resolve
   tmux off the process-global `PATH` but took no env lock, so they raced
   `TaskLaunchEnv`'s fake tmux and failed intermittently. This reproduces on
   clean main (1 of 4 runs), independently of this branch's tests — the four new
   tests only shift scheduling enough to expose it more often (2 of 3 runs).
   Each now takes `journal::test_env_lock()`, matching the convention main's own
   `TaskLaunchEnv` callers already follow. `ops::task::tests` is 3/3 green after,
   and the full `cargo test -p loopflow` suite passes.

   This is the one deliberate step outside the branch's scope. It touches only
   test setup, no production code. If a reviewer prefers it as its own fix, the
   three `_env_lock` lines lift out cleanly — but reverting them restores
   intermittent CI failures unrelated to this branch.
