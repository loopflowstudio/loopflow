# Design Review: Local Notifications and Unified Execution

Branch: `jack-heart.concerto-next.20260129_2255.20260129_2318.aurora-rondo`

## Summary

This branch adds local macOS notifications for wave state changes and unifies step execution logic. It also removes the work queue feature and fixes branch naming for nested timestamps.

## What Changed

### 1. Local Notifications for Waves

**Files:** `swift/LoopflowCore/Services/NotificationService.swift`, `swift/Concerto/State/RepoState.swift`

Waves now trigger macOS notifications when they need attention:
- **Interactive step waiting** - "Wave waiting: step-name"
- **Error/failure** - "Wave failed" with truncated error message
- **PR ready** - "Wave PR #123 ready for review"

Implementation:
- `NotificationService` is a singleton wrapping `UNUserNotificationCenter`
- Uses `threadIdentifier = waveId` so notifications group per-wave
- Clicking a notification selects the wave and brings the app to front
- Notifications show even when app is in foreground (via delegate)

`RepoState` tracks previous wave statuses and fires notifications on transitions:
- `previousWaveStatuses: [String: WaveStatus]`
- `handleWaveStatusChange()` compares old vs new and calls appropriate notify method

### 2. Unified Step Execution

**Files:** `src/loopflow/lf/execution.py` (new), `src/loopflow/lf/step.py`, `src/loopflow/lf/flow.py`, `src/loopflow/lf/output.py`

Step execution was duplicated between `step.py` (single-step) and `flow.py` (flow execution). Now unified in `execution.py`:

```python
@dataclass
class ExecutionParams:
    step_name: str
    repo_root: Path
    components: list[Component]
    backend: Backend
    # ... other params
```

`execute_step(params)` handles both interactive and auto modes, used by both single-step CLI and flow runner.

Benefits:
- Single source of truth for step execution
- Consistent behavior between `lf step` and `lf flow`
- Easier to add execution features (they apply everywhere)

### 3. Work Queue Removal

**Deleted:**
- `src/loopflow/lfwork/` (entire module)
- `src/loopflow/lfd/work/` (Asana and file backends)
- `swift/LoopflowCore/Models/WorkItem.swift`
- `swift/Concerto/Views/WorkItemViews/`
- Related tests

The work queue feature was not being used. Removing it simplifies the codebase.

### 4. Branch Naming Fix

**Files:** `src/loopflow/lf/naming.py`, `tests/test_naming.py`

`parse_branch_base()` now handles nested timestamps recursively:

```
foo.20260127_2204.aurora-melody.20260127_2210.wisp-forte → foo
```

This happens when a wave is created from another wave's branch (nested iterations).

### 5. Minor Improvements

- `lfd reset` command to reinitialize the database
- `_resolve_wave()` helper for wave name or ID resolution in CLI
- Orphaned branch cleanup in `worktrees.create()`
- `print_step_header()` for consistent step display

## Key Choices

**Singleton NotificationService:** Matches Apple's pattern for `UNUserNotificationCenter`. The delegate must be set early and persist.

**Polling for status changes:** `RepoState` polls wave statuses periodically. Comparing old vs new is simpler than event-driven notification and works with the existing polling architecture.

**Unified execution via dataclass:** `ExecutionParams` bundles all the parameters needed for step execution. This is cleaner than passing 10+ individual arguments.

## How It Fits Together

```
User launches Concerto
    ↓
ConcertoApp.init requests notification authorization
    ↓
RepoState polls waves via lfd
    ↓
On status change: RepoState.handleWaveStatusChange()
    ↓
NotificationService.notifyError/notifyNeedsInteractive/notifyPRReady
    ↓
User clicks notification
    ↓
NotificationService delegate posts .selectWave to NotificationCenter
    ↓
WaveSidebar receives notification, selects wave
```

## Risks

**Notification overload:** If many waves change status at once, user gets many notifications. Mitigated by `threadIdentifier` grouping per-wave.

**Polling delay:** Status changes won't trigger notifications until the next poll. Current polling interval is reasonable for the use case.

## What's Not Included

- Notification preferences (enable/disable per type)
- Custom notification sounds
- Badge count on app icon
- Notification actions (buttons to take action directly from notification)

These can be added later if needed.

## Test Results

- Python: 668 passed
- Swift: 61 passed
- Concerto build: succeeded
