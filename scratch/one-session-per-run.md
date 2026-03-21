# One Session Per Run

## The change

The daemon stops orchestrating flows step-by-step. Instead, it creates one tmux session per wave run, launches `lf <flow>` inside it, and monitors the session. The CLI handles all flow sequencing — xor routing, loops, fork, interactive steps — in-process, inside that session.

## What goes away (~1,500 lines)

### Daemon step orchestration (`wave/mod.rs`)
- `DaemonFlowExecutor` struct + `StepExecutor` impl (~190 lines)
- `execute_step_process` (~30 lines)
- `load_execution_cursor` / `store_execution_cursor` (~25 lines)
- `advance_run_step` (~12 lines)
- `run_step` / `run_step_without_fast_path` (~70 lines)
- `TempStepGuard` / `write_temp_step` (~30 lines)

### Waiting/resume machinery (`wave/mod.rs`)
- `spawn_terminal_session_watcher` / `wait_for_terminal_session_and_resume` / `wait_for_terminal_session_status` / `resume_run_execution` (~130 lines)
- `create_terminal_session` / `create_terminal_session_for_waiting_wave` (~100 lines)
- `WaveRunStatus::Waiting` and all branches on it

### Daemon-side fork (`wave/fork.rs`)
- Entire file (~390 lines). The CLI's `run_and` already creates worktrees, runs branches in parallel threads, and cleans up reliably.
- `ForkRun` store records and all fork store queries
- `cleanup_orphaned_fork_runs` in the worktree janitor

### HTTP routes
- `continue_wave_handler` (~115 lines) — no Waiting state to continue from

### Env vars
- `LFD_RUN_ID` — redundant with `LF_RUN_ID`, never read

### DB
- `execution_cursor` column on `wave_runs` — leave inert, no migration needed

## What `execute()` becomes

```
1. Load run + wave
2. Spawn git state poller
3. Build `lf <flow> -b` command with env (LFD_WAVE_ID, LF_RUN_ID)
4. Launch in a tmux session (reuse launch_tmux_terminal_session)
5. Monitor session (reuse monitor_tmux_terminal_session)
6. On exit 0: finish_completed_run
7. On exit != 0: fail_run
```

One `TerminalSession` record per run. The `step` field carries the flow name.

## What stays

- **Scheduling + triggers**: `spawn_run_task_with_slot`, activation, cron, loop ticker, recovery — all unchanged. They call `executor.execute(run_id)`, don't care what happens inside.
- **Run lifecycle**: `finish_completed_run`, `fail_run`, `create_repair_run`, `trigger_listeners_on_completion` — unchanged.
- **Worktree janitor**: stays, minus fork worktree cleanup (CLI handles that now).
- **Git state poller**: stays, scoped to session lifetime.
- **`FlowEngine` + `ExecutionCursor`** in `engine/execution.rs`: stays — the CLI uses them. The daemon stops using them.
- **`launch_tmux_terminal_session` / `monitor_tmux_terminal_session`**: repurposed for run-level sessions.

## How interactive steps work

The human attaches to the run's tmux session. When `lf build` hits an interactive step like `design`, the agent runs in the tmux pane and waits for input. The human is there (or attaches). When they're done, the flow continues to the next step automatically.

No `Waiting` status. No cursor persistence. No resume. The flow just runs.

## How fork works

The CLI's `run_and` (in `lf/commands/flow.rs`) already:
- Creates worktrees per branch
- Runs branches in parallel threads
- Pipes output with `[fork-N]` prefixes
- Writes the fork manifest
- Runs the synthesize step
- Cleans up all worktrees

This runs inside the tmux session like any other flow item.

## Per-step progress

The daemon currently knows which step is running because it orchestrates them. Under this model, `lf` emits journal events from inside the session — `Started`/`Completed`/`Errored` for both flows and individual steps. The daemon observes progress through the journal. Structured observation, not orchestration.

## Migration

- Old `Waiting` runs in the DB: startup recovery fails them.
- `execution_cursor` column: left inert. No schema migration needed.
- `ForkRun` table: can be dropped in a future migration.
