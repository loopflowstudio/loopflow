# Branch review: jack-heart.conviction.20260209_1243

## What was implemented

Three independent improvements shipped together:

1. **Ctrl+C child cleanup** — `lf` now sends SIGTERM to the child agent before exiting on Ctrl+C. Previously the agent process could survive as an orphan.

2. **Wave event emissions** — The executor and HTTP handlers now emit `wave_updated` events when steps advance, and when `continue`, `land`, `next`, `collapse`, and `absorb` complete. This drives real-time UI updates in Concerto.

3. **Nearest-base-ref for stacked diffs** — `infer_wave_git_state` now picks the closest ancestor branch for diff comparison instead of always diffing against `origin/main`. Handles stacking: after `next`, the previous iteration branch is the closest ancestor.

Plus cleanup: `kill_process` deduplicated from `executor` into `engine::platform`, stale scratch files removed.

## Key choices

**AtomicU32 for child PID** — Signal handlers can't use locks. `AtomicU32` with Acquire/Release ordering is the simplest correct approach. PID 0 means "no child."

**SIGTERM via `kill` command** — Rather than using `libc::kill` directly, the implementation shells out to `kill -TERM`. Matches the existing `open_url` pattern in `engine::platform` and avoids unsafe code.

**`nearest_base_ref` returns a commit SHA, not a branch name** — The merge-base SHA is the actual diff point. Returning the SHA directly avoids an extra rev-parse and is correct regardless of whether the branch has since moved.

**Raw git commands in route helpers** — `wave_remote_branches` and `commit_count` use `std::process::Command` directly rather than `engine::git` helpers. This matches the existing `git_commit_log` and `git_diff_stat` functions in the same file, which pre-date this branch.

## How it fits together

The child PID tracking is self-contained: `agent.rs` stores the PID in an atomic on spawn, clears it on exit, and exposes `kill_child_if_running()` for the signal handler.

The event emissions follow a consistent pattern: after any state mutation completes, call `event_hub.send(Event::wave_updated(...))`. The executor does this in `execute()` after each step advance; HTTP handlers do it after their operations complete.

The nearest-base-ref logic runs inside `infer_wave_git_state`, which is called when building wave DTOs. It collects candidate branches (default branch + remote siblings), finds the merge-base for each, and picks the closest one by commit count.

## Risks and bottlenecks

- **`nearest_base_ref` runs git commands synchronously** on `spawn_blocking`. For repos with many remote branches, `git branch -r` could be slow. In practice, waves produce a handful of branches, so this is bounded.
- **Race between SIGTERM and process exit** — If the child exits between PID load and `kill`, SIGTERM hits a dead PID (harmless) or a recycled PID (extremely unlikely in the time window). Acceptable tradeoff vs. adding process group management.
- **Event fan-out** — More `wave_updated` events means more UI refreshes. Each event triggers a GET /waves/:id fetch. With fast step execution, this could cause brief bursts of API traffic. Not a problem at current scale.

## What's not included

- No UI changes in Concerto for the step progress pills — those were part of a prior branch. This branch provides the backend plumbing (events + step_index) they consume.
- No unit test for `nearest_base_ref` itself — it depends heavily on git state. The executor test covers the event emission path. Integration coverage comes from the e2e tests.
- No migration of remaining raw git commands in `routes/mod.rs` to `engine::git` — out of scope for this branch, and matches existing patterns.
