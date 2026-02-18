# 01D Hardening — Design Review

## What was implemented

Phase 01D hardens the Docker-backed remote execution path. The changes fall into six categories:

1. **Shared fork-selection resolver** (`engine/fork.rs`). A single `plan_fork_execution()` function resolves `ForkSelect::One`, `Prompt`, and `All` for both CLI and daemon, eliminating behavioral drift. Headless `Prompt` fails fast instead of silently auto-selecting.

2. **RAII scheduler slot management** (`lfd/scheduler.rs`). `SchedulerSlotGuard` releases slots on drop, making slot release cancellation-safe. Idempotent `acquire_guard()` returns a no-op guard when the same run re-acquires its slot.

3. **Fork execution in the wave executor** (`lfd/executor/wave/fork.rs`). Multi-branch forks run in parallel via tokio tasks, each acquiring its own slot. Results are collected via mpsc; first failure cancels siblings. Cleanup removes worktrees and DB records unconditionally.

4. **Startup orphan cleanup** (`lfd/executor/wave/mod.rs`). On daemon restart, orphaned fork runs (DB records whose parent wave run has already terminated) are cleaned — worktrees removed, records deleted. Both sqlite and postgres stores implement `list_orphaned_fork_runs()` and `delete_fork_runs()`.

5. **Agent execution timeout** (`lfd/config.rs`, `lfd/executor/local.rs`). Configurable via `executor.agent_timeout` (default 45m) with `LFD_EXECUTOR_AGENT_TIMEOUT` env override. Accepts humantime duration strings ("30m", "2h"). Enforced by `tokio::time::timeout()` in the local executor.

6. **CI promotion** (`.github/workflows/`). Docker smoke tests are now a required PR job. A nightly workflow runs recovery, fork hardening, and smoke tests with `--nocapture` for debugging.

## Key choices

**Single fork resolver, not per-runtime.** CLI and daemon previously had separate fork-selection logic. Consolidating into `engine/fork.rs` makes the behavior identical and testable in isolation. The daemon passes `None` for the prompt chooser (headless), which correctly fails for `ForkSelect::Prompt`.

**RAII over explicit release.** The previous pattern required callers to remember to release scheduler slots on every error path. `SchedulerSlotGuard` makes release automatic. The idempotent re-acquire pattern (`release_on_drop: false`) avoids double-release when a fork branch's slot was already held by its parent run.

**fork(select=all) blocked on Docker.** The Docker executor doesn't support worktree-based forking because containers mount a single volume. Rather than silently degrading, this returns a clear error. The local executor handles it via git worktrees.

**Timeout is operator config, not watchdog.** Making the timeout explicit and configurable avoids surprising behavior. Operators can tune it per-environment via the YAML config or env var.

## How it fits together

```
engine/fork.rs          — plan which branches to execute, write manifest
lfd/scheduler.rs        — idempotent slot acquisition with RAII guard
lfd/executor/wave/      — orchestrates flow execution
  fork.rs               — parallel fork branch execution (tokio tasks + mpsc)
  launch.rs             — single agent launch lifecycle
  sidecar.rs            — CI fix agent spawn with slot guard
  mod.rs                — main execute() loop, orphan cleanup, worktree janitor
lfd/config.rs           — mode profiles, timeout config, credential mounts
lfd/triggers/           — cron/watch/loop all use acquire_guard + spawn_run_task_with_slot
lfd/store/{sqlite,pg}   — fork_runs table for tracking parallel branches
```

## Risks and bottlenecks

- **Docker build-context tar creation** can be expensive for large repos. Not addressed in this branch; called out for follow-up.
- **Fork cleanup is best-effort.** If a worktree removal fails, it logs a warning and continues. The worktree janitor provides a second line of defense but runs on-demand, not periodically.
- **No hard-stop cancellation** for already-running fork branches. Cancellation sets a token; branches check it between phases but a long-running agent won't be interrupted mid-execution.

## What's not included

- Full flow checkpoint/restore across daemon restarts.
- Downtime log replay/backfill.
- Per-wave credential scoping (current: global credential mounts).
- Docker network policy redesign.
- Hosted orchestration (phases 07-09).
- Docker build-context optimization for large repos (planned follow-up).
