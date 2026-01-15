# Task History for Worktrees

Surface task history per worktree so users can see what stage each branch is in (design → implement → review → etc).

## What to build

A history system where `lf` logs task executions to `lfd`, and Maestro queries that history to show each worktree's progression through tasks.

## Architecture

```
┌─────────┐        ┌─────────┐        ┌─────────┐
│  lf     │──log──▶│  lfd    │◀─query─│ Maestro │
│ (CLI)   │        │(daemon) │        │ (macOS) │
└─────────┘        └─────────┘        └─────────┘
```

Currently `lf` logs to `maestro.db` directly. This changes to:
1. `lf` sends session events to `lfd` via Unix socket
2. `lfd` stores in `lfd.db` (already has a `sessions` table)
3. Maestro queries `lfd` for session history per worktree

## Data structures

`lfd.db` already has a `sessions` table. Add history querying:

```python
# src/loopflow/lfd/db.py

def load_sessions_for_worktree(worktree: str, limit: int = 20) -> list[Session]:
    """Load recent sessions for a worktree path."""
    ...

def load_sessions_for_repo(repo: str, limit: int = 50) -> list[Session]:
    """Load recent sessions across all worktrees in a repo."""
    ...
```

Add methods to `lfd` server:

```python
# src/loopflow/lfd/server.py

# New method: sessions.history
async def _handle_sessions_history(self, params: dict) -> Response:
    """Return session history for a worktree or repo.

    params:
        worktree: str (optional) - filter to specific worktree
        repo: str (optional) - filter to repo
        limit: int (default 20) - max sessions to return
    """
    ...
```

## Key functions

```python
# src/loopflow/lfd/client.py

def log_session_start(session: Session) -> None:
    """Tell lfd a session started. Fire-and-forget."""
    ...

def log_session_end(session_id: str, status: SessionStatus) -> None:
    """Tell lfd a session ended."""
    ...

def get_worktree_history(worktree: str, limit: int = 20) -> list[Session]:
    """Get session history for a worktree."""
    ...
```

```python
# src/loopflow/cli/run.py changes

# In _execute_task, replace direct maestro.db writes:
# Before: save_session(DEFAULT_DB_PATH, session)
# After:  lfd_client.log_session_start(session)
```

Swift side (Maestro):

```swift
// Maestro/Models/Session.swift (new file)

struct TaskSession: Identifiable, Codable {
    let id: String
    let task: String
    let status: String  // running, completed, error
    let startedAt: Date
    let endedAt: Date?
    let model: String
}
```

```swift
// Maestro/Services/SessionService.swift (new file)

struct SessionService {
    func history(for worktree: String, limit: Int = 20) async throws -> [TaskSession]
}
```

```swift
// Maestro/Models/Worktree.swift - extend

struct Worktree {
    // existing fields...
    var recentTasks: [TaskSession]  // populated from lfd

    var lastTask: String? {
        recentTasks.first?.task
    }

    var stageText: String {
        // "design" / "implement" / "review" based on last completed task
    }
}
```

## UI changes

WorktreeRow shows stage indicator:

```swift
// Before: statusBadge shows "clean" / "modified"
// After:  also show last task as stage indicator

private var stageBadge: some View {
    if let task = worktree.lastTask {
        Text(task)
            .font(.caption2)
            .padding(.horizontal, 6)
            .padding(.vertical, 2)
            .background(stageColor(task).opacity(0.2))
            .foregroundStyle(stageColor(task))
            .clipShape(Capsule())
    }
}

private func stageColor(_ task: String) -> Color {
    switch task {
    case "design": return .blue
    case "implement": return .purple
    case "review": return .orange
    case "polish": return .green
    default: return .gray
    }
}
```

Add history popover on click:

```swift
// Click worktree row -> show history popover with recent tasks
// Each entry shows: task name, status icon, relative time
```

## Constraints

- **lfd must be running** for history to work. If not running, `lf` falls back to direct file logging (no-op to lfd).
- **Fire-and-forget logging** - task execution shouldn't block on lfd communication.
- **Keep maestro.db for Maestro's own state** (agents, etc). Sessions move to lfd.db.

## Done when

1. Run a task: `lf review`
2. Query lfd: `echo '{"method": "sessions.history", "params": {"limit": 5}}' | nc -U ~/.lf/lfd.sock`
3. See the session in output with task name, worktree, status, timestamps
4. In Maestro, worktree row shows the task name as a stage badge
