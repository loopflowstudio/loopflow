# agents1

Implements background loop execution for `lfd loop`. Loops now spawn as background processes, run iterations until the PR limit is hit, then pause in WAITING state.

## Review

**Verdict:** Ready to ship

The implementation is solid and follows the design docs closely. A few observations that don't block shipping:

1. **Scheduler is unused.** `scheduler.py` was added (good implementation, thread-safe) but nothing calls it yet. The current loop runner checks PR limits per-loop via `count_outstanding()`, which works for single loops. The scheduler is infrastructure for parallel loops—future work per `.design/parallel-loops.md`. Not a bug, just scaffolding.

2. **`loop_runner.py:167-186` prompt injection.** The goal content injection via `_replace(task=...)` works but modifies a namedtuple by replacing the whole `task` field. If `components.task` is `None`, this silently produces a malformed prompt. In practice the task file always exists for configured pipelines, but worth noting.

3. **Signal handling in stop.** `stop_loop()` sends SIGTERM/SIGKILL but doesn't wait for the process to actually terminate. Subsequent `start_loop()` could race with a slow-dying process. The `is_process_running()` check handles most cases, but there's a window.

4. **Test coverage.** Database and model tests are thorough. `StartResult`, `count_outstanding`, and the new CLI flags all have tests. Missing: integration tests for `loop_runner.py` (would require mocking subprocess, worktree creation, and gh CLI).

## Design notes

**Branch model:** Each loop gets a `{goal}-main` branch. Iterations create `{goal}/{iteration:03d}` branches from personal-main, PR back to personal-main, auto-merge, cleanup. `lfops land --squash` merges personal-main to real main.

**Outstanding counting:** `git rev-list --count origin/main..origin/{personal_main}` after fetching. Returns 0 if branch doesn't exist yet.

**Background execution:** `lfd loop test-coverage` spawns `python -m loopflow.lfd.loop_runner {loop_id}` detached via `start_new_session=True`. PID stored in DB for stop/status.

**Waiting state:** When `outstanding >= pr_limit`, loop sets status=WAITING and exits. Human must `lfops land --squash` to clear the queue, then re-run `lfd loop` to resume.
