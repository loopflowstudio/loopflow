# Branch Review: jack-heart.lfd-cli.20260128_1203

## What was implemented

This branch delivers two major changes:

1. **Unified step execution** — Extracted shared execution logic from `step.py` and `flow.py` into a new `execution.py` module. Both single-step (`lf <step>`) and flow execution (`lf flow`) now use the same `execute_step()` function, eliminating duplicated command-building, session creation, and collector invocation code.

2. **Removal of work queue feature** — Deleted the entire work queue system (`lfwork` CLI, `lfd/work/` backends, Swift models and views). This feature was experimental and never reached production use.

## Key choices

**Why unify execution?**

The previous code had nearly identical execution logic in three places: `_execute_step()` in step.py, `_run_step()` in flow.py, and `_run_interactive_step()` in flow.py. Each built commands, created step runs, handled environment variables, and invoked the collector slightly differently. Changes required updating all three.

The new `ExecutionParams` dataclass captures all execution parameters, and `execute_step()` handles both interactive and auto modes. The `use_execvp` flag differentiates single-step (replaces process) from flow (subprocess.run).

**Why remove work queue?**

The work queue (`lfwork`) was designed to integrate with Asana/file-based task queues, but:
- Never shipped to users
- Asana backend required API tokens most users don't have
- Replaced by the simpler wave/flow model
- Carried maintenance cost for code nobody used

**Output header consolidation**

Added `print_step_header()` to `output.py` for consistent step display across single-step and flow execution. Shows step name, model config, direction, and token summary in a unified format.

## How it fits together

```
lf <step>  ─────┐
                │
                ▼
lf flow ────► execute_step() ───► _execute_interactive() ─► subprocess.run()
                │                                              or os.execvp()
                │
                └───────────────► _execute_auto() ─────────► collector subprocess
```

Both entry points gather components, then call `execute_step()` with appropriate `ExecutionParams`. The function handles session creation, output formatting, and dispatches to the right execution mode.

## Risks and bottlenecks

**Process replacement behavior** — Single-step interactive mode still uses `os.execvp()` which replaces the process. This is intentional (preserves TTY properly) but means errors in the new unified path could affect the user experience more visibly.

**Orphaned branch handling** — The worktree changes now auto-delete orphaned branches (branches without worktrees). This is helpful for cleanup but could surprise users who expected their branch to persist.

## What's not included

**Token summary in auto mode** — Previously the step.py version passed `--token-summary` to the collector; the new unified version doesn't. This was removed because the summary is already printed by `print_step_header()` before execution starts.

**Backwards compatibility for work queue config** — Old config files with `work:` keys will fail validation since the config field was removed. This is intentional per CLAUDE.md guidelines (no backwards compat for internal config).

## Files changed

| Change type | Files |
|-------------|-------|
| Added | `src/loopflow/lf/execution.py` (new unified execution module) |
| Modified | `src/loopflow/lf/step.py` (simplified, delegates to execution.py) |
| Modified | `src/loopflow/lf/flow.py` (simplified, delegates to execution.py) |
| Modified | `src/loopflow/lf/output.py` (added print_step_header) |
| Modified | `src/loopflow/lf/worktrees.py` (orphaned branch cleanup) |
| Modified | `src/loopflow/lf/config.py` (removed WorkConfig) |
| Modified | `src/loopflow/lfd/cli.py` (added reset command, wave name resolution) |
| Deleted | `src/loopflow/lfd/work/` (entire work queue system) |
| Deleted | `src/loopflow/lfwork.py` (work queue CLI) |
| Deleted | `tests/test_work.py` (work queue tests) |
| Deleted | `swift/` work-related files (WorkItem, WorkService, WorkQueueView) |

## Test coverage

- 666 Python tests pass
- 9 Rust tests pass
- Cargo fmt and clippy clean
- No new test coverage added for execution.py since it's a refactor of existing tested code paths
