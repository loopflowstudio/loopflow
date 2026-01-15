# History

Migrate session logging from `maestro.db` to `lfd` daemon, enabling worktree-based history queries and live UI updates in Maestro.

## Implementation

Session logging now flows through lfd:
- `cli/run.py`, `pipeline.py`, and `maestro/runner.py` use fire-and-forget calls to lfd via `log_session_start()` and `log_session_end()`
- lfd server handles `sessions.start`, `sessions.end`, `sessions.history` methods
- Maestro reads session history directly from `lfd.db` via `SessionService.swift`

The `loopflow.lfd.models.Session` is the canonical session model:
- Uses `str` for `repo` and `worktree` (cleaner for JSON/SQLite)
- Uses `model` field instead of `backend`
- The old `maestro.session.Session` remains for backwards compatibility with agent-related code

## Components

### Python (lfd)
- `loopflow/lfd/models.py` - Session model with serialization
- `loopflow/lfd/client.py` - Fire-and-forget functions: `log_session_start()`, `log_session_end()`
- `loopflow/lfd/server.py` - Handles `sessions.*` methods, broadcasts events
- `loopflow/lfd/db.py` - SQLite storage with history queries by worktree/repo

### Swift (Maestro)
- `Models/Session.swift` - TaskSession model matching lfd schema
- `Models/Worktree.swift` - Extended with `recentTasks: [TaskSession]`
- `Services/SessionService.swift` - Reads from `~/.lf/lfd.db`
- `Views/WorktreeSidebar.swift` - Shows stage badges from last completed task

## Decisions

- **Fire-and-forget**: Session logging never blocks task execution. If lfd isn't running, logging fails silently.
- **Direct DB reads**: Maestro reads `lfd.db` directly rather than querying lfd socket. Simpler for read-only queries.
- **Dual Session models**: The old `maestro.session.Session` is kept for backwards compatibility in agent-related code (`maestro/__init__.py` exports it). Task execution uses the new `lfd.models.Session`.

## Not migrated

These still use `maestro.db` directly for agent state (not session history):
- `cli/status.py`, `cli/sessions.py` - Display running sessions
- `lfops.py` - Session status display
- `maestro/agents.py`, `maestro/agent_runner.py` - Agent state management

This is intentional - agent state lives in maestro.db, session history lives in lfd.db.
