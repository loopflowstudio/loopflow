# Gate Review: Git Sync Hardening

## What was implemented

Hardened the executor's git sync layer against concurrent push failures. Five changes across the sync functions in `helpers.rs`, a new `ops/agent.rs` module, and timeout support in `engine/agent.rs`.

1. **Dual rebase in `pre_step_sync`.** Now rebases onto both `origin/{branch}` and `origin/{default_branch}` at every step boundary, matching what `sync_existing_worktree` already did for worktree creation. Extracted a shared `dual_rebase` helper used by both call sites.

2. **Rebase conflict recovery.** All three bare `rebase()` call sites replaced with `rebase_with_recovery()`, routing conflicts through the rebase agent instead of aborting silently.

3. **Shared `run_builtin_agent` helper.** New `ops/agent.rs` extracts the "load builtin step, gather context, launch headlessly" pattern from `run_rebase_agent`. Both rebase recovery and push escalation use it. `run_rebase_agent` is now a 5-line wrapper.

4. **Push failure escalation.** After the existing fetch+rebase+retry cycle fails, `post_step_sync` now escalates to a debug agent session instead of hard-failing. Rich error messages include original error, retry error, and agent error for diagnostics.

5. **Agent session timeouts.** `ProcessConfig` gains an optional `timeout` field. `launch_agent` enforces it via `child.try_wait()` polling (batch/interactive) or `recv_timeout` polling (streaming). `child.kill()` is called through the `Child` handle directly, avoiding the PID-reuse race the design review flagged. Rebase agent: 30 min. Debug agent: 5 min.

## Key choices

| Decision | Why |
|----------|-----|
| `rebase_with_recovery` everywhere | Silent failures were the worst part of 03.5 — a step running on stale code wastes compute and produces commits that won't merge |
| `dual_rebase` shared helper | `pre_step_sync` and `sync_existing_worktree` were drifting. One function, called from both, eliminates that |
| `run_builtin_agent` in `ops/agent.rs` | Avoids duplicating the 40-line agent launch sequence between rebase and push escalation |
| `TracingProgress` over `NullProgress` | Executor sync is the one place where visibility into rebase/agent activity matters most |
| Poll-based timeout via `try_wait`/`recv_timeout` | Avoids `Arc<Mutex<Child>>` complexity. Clean: poll, kill if expired, drain remaining output |
| `RebaseResult` removed | Return type was always `Ok(RebaseResult { success: true })` — the success field carried no information. Simplified to `OpsResult<()>` |

## How it fits together

```
Step boundary
  → pre_step_sync(main_repo, worktree, branch)
    → dual_rebase
      → rebase_onto_if_available(wave branch)   [rebase_with_recovery]
      → rebase_onto_if_available(default branch) [rebase_with_recovery]

Step executes...

  → post_step_sync(worktree, branch, step_name)
    → commit + push
    → on push failure: fetch + rebase_with_recovery + retry push
    → on retry failure: run_builtin_agent("debug") + final push attempt
```

`run_builtin_agent` is the shared primitive. It loads a builtin step by name, gathers repo context, formats a headless prompt, and launches the configured agent with an optional timeout.

## Risks and bottlenecks

- **Agent API dependency.** Rebase recovery and push escalation both need agent API availability. An outage during sync fails the run — same as today's hard-fail, not worse.
- **Latency at step boundaries.** Agent sessions add latency when conflicts occur (up to 30 min for rebase, 5 min for debug). This is intentional — resolving conflicts is real work that would otherwise require manual intervention.
- **Double fetch.** `rebase_onto_if_available` fetches from `main_repo`, then `rebase_with_recovery` fetches again from `worktree`. Harmless since worktrees share the gitdir, but redundant.

## What's not included

- Concurrency limiting for listen fan-out (separate concern)
- Push retry backoff (agent escalation replaces this)
- Changes to `rebase_with_recovery` internals (works as-is)
- Agent rate limiting

## Test coverage

- `pre_step_sync_rebases_onto_wave_and_default_branch` — verifies dual rebase picks up both wave branch and default branch changes
- `pre_step_sync_skips_missing_remote_branch_and_still_rebases_default` — verifies graceful handling when wave branch has no remote
- `launch_batch_times_out` — verifies timeout kills batch agent and returns error
- `launch_streaming_times_out` — verifies timeout kills streaming agent and returns error
- All existing rebase tests updated for `OpsResult<()>` return type
