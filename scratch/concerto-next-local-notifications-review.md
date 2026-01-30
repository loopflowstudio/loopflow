# Design Review: Local Notifications + Unified Execution

## What was implemented

### 1. Local notifications for wave state changes

macOS notifications surface when waves need attention, fail, or complete with PRs. Tap a notification → focus Concerto → select that wave.

Three notification types:
- **Needs Interactive** — Wave waiting for user input
- **Error** — Wave failed
- **PR Ready** — Wave completed, PR awaiting review/land

### 2. Unified step execution

Extracted shared execution logic from `step.py` and `flow.py` into `execution.py`. Both single-step (`lf <step>`) and flow execution (`lf flow`) now use the same `execute_step()` function.

Before: Three near-identical code paths for building commands, creating step runs, and invoking collector.

After: Single `ExecutionParams` dataclass + `execute_step()` function handles both interactive and auto modes.

### 3. Removal of work queue system

Deleted the entire `lfwork` feature:
- `src/loopflow/lfwork.py` (CLI)
- `src/loopflow/lfd/work/` (backends: asana, file)
- `swift/` work-related files (WorkItem, WorkService, WorkQueueView)
- `tests/test_work.py`
- Removed `asana` dependency from `pyproject.toml`

This feature was experimental, never shipped, and replaced by the wave/flow model.

### 4. Branch naming fix

`parse_branch_base()` now handles nested timestamps recursively, fixing cases like `foo.20260129_2255.20260129_2318.aurora-rondo`.

### 5. lfd reset command

Added `lfd reset` to stop all waves, delete the database, and reinitialize with the latest schema.

## Key choices

| Decision | Why |
|----------|-----|
| `NotificationService` singleton | Single delegate for `UNUserNotificationCenter` required by Apple |
| Status change detection in `RepoState.refreshWaves()` | Already polls wave state; comparing old vs new avoids separate event stream |
| `threadIdentifier = wave.id` | Groups notifications per wave, newer replaces older |
| `ExecutionParams` dataclass | Captures all execution parameters cleanly; `use_execvp` flag differentiates single-step (replaces process) from flow (subprocess.run) |
| Delete work queue entirely | Feature never shipped, carried maintenance cost, replaced by waves |
| Recursive timestamp stripping | Wave branches can accumulate multiple iteration suffixes from stacking |

## How it fits together

### Notifications

```
Wave status changes on server
         │
         ▼
RepoState.refreshWaves() polls via WaveService
         │
         ▼
handleWaveStatusChange() compares old/new status
         │
         ▼
NotificationService.shared.notify*() → UNUserNotificationCenter
         │
         ▼
User taps notification → .selectWave posted → WaveSidebar selects wave
```

### Unified execution

```
lf <step>  ─────┐
                │
                ▼
lf flow ────► execute_step() ───► _execute_interactive() ─► subprocess.run()
                │                                              or os.execvp()
                │
                └───────────────► _execute_auto() ─────────► collector subprocess
```

## Risks and bottlenecks

**Notification permission** — Requested on app launch. If denied, notifications silently fail.

**Polling-based detection** — Status changes detected on next poll (~5s). Near-instant for practical use, but not real-time.

**Process replacement** — Single-step interactive mode uses `os.execvp()` which replaces the process. Errors in the unified path affect user experience directly.

**No work queue migration** — Old config files with `work:` keys will fail validation. Per CLAUDE.md, no backwards compat for internal config.

## What's not included

- Remote push notifications (Phase 2, requires Loopflow account + APNS)
- Fine-grained notification preferences (quiet hours, per-wave toggles)
- Action buttons on notifications (approve/dismiss from notification)
- Rich notifications with images

## Test coverage

| Suite | Result |
|-------|--------|
| Python | 668 tests pass |
| Swift package | 61 tests pass |
| Concerto build | succeeds |

Tests added:
- `test_parse_branch_base_strips_trailing_timestamp`
- `test_parse_branch_base_nested_timestamps`

## Files changed

| Category | Files |
|----------|-------|
| **Added** | `execution.py` (unified step execution), `NotificationService.swift` |
| **Modified** | `step.py`, `flow.py`, `output.py`, `naming.py`, `cli.py`, `RepoState.swift`, `WaveSidebar.swift`, `ConcertoApp.swift`, `LoopflowConfig.swift`, `ConfigLoader.swift` |
| **Deleted** | `lfwork.py`, `lfd/work/*`, `WorkItem.swift`, `WorkService.swift`, `WorkQueueView.swift`, `test_work.py` |
