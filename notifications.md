# Notifications & Session Tracking

Centralized agent tracking and notification system for loopflow.

## What to build

A `maestro` daemon that tracks running sessions (interactive and background), emits notification events, and provides status visibility across terminals.

## Core quotes

> "I want lf to manage both parallel interactive or short-command-sequential co-piloting sessions and background agents"

> "We just want a centralized agent tracker and notification system with maybe only a few entry points to start"

> "If the maestro is on, the interactive sessions should register themselves, but not complain if not running"

## Data structures

```python
from dataclasses import dataclass
from datetime import datetime
from enum import Enum
from pathlib import Path
from typing import Literal


class SessionStatus(Enum):
    RUNNING = "running"
    WAITING = "waiting"      # needs input/permission
    COMPLETED = "completed"
    ERROR = "error"


@dataclass
class Session:
    id: str                          # unique identifier (uuid)
    task: str                        # task name or "inline"
    repo: Path                       # main repo root
    worktree: Path                   # where session is running
    status: SessionStatus
    started_at: datetime
    pid: int | None = None           # for background agents
    backend: str = "claude-code"     # future: other backends


class NotificationType(Enum):
    STARTED = "started"
    WAITING = "waiting"       # agent needs input
    COMPLETED = "completed"
    ERROR = "error"


@dataclass
class NotificationEvent:
    type: NotificationType
    session_id: str
    message: str
    timestamp: datetime
    repo: Path                # for routing/filtering
```

## Maestro service

Background daemon that:
- Listens on Unix socket (`~/.lf/maestro.sock`)
- Tracks active sessions in memory
- Persists state to `~/.lf/maestro.json` for recovery
- Dispatches notifications to configured adapters

```python
class Maestro:
    sessions: dict[str, Session]
    adapters: list[NotificationAdapter]

    def register_session(self, session: Session) -> None: ...
    def update_status(self, session_id: str, status: SessionStatus) -> None: ...
    def unregister_session(self, session_id: str) -> None: ...
    def list_sessions(self, repo: Path | None = None) -> list[Session]: ...
    def emit(self, event: NotificationEvent) -> None: ...
```

### Communication protocol

Simple JSON over Unix socket:

```python
# Request
{"action": "register", "session": {...}}
{"action": "update", "session_id": "xxx", "status": "completed"}
{"action": "list", "repo": "/path/to/repo"}  # repo optional

# Response
{"ok": True}
{"ok": True, "sessions": [...]}
{"ok": False, "error": "..."}
```

## Notification adapters

```python
class NotificationAdapter:
    def send(self, event: NotificationEvent) -> None: ...

class MacOSAdapter(NotificationAdapter):
    """osascript display notification"""

class ClaudeCodeHooksAdapter(NotificationAdapter):
    """Integrate with Claude Code's hook system"""
    # Future: register hooks that call back to maestro
```

Configuration in `~/.lf/config.yaml`:

```yaml
notifications:
  adapters:
    - macos           # always on by default
    - claude-hooks    # optional integration
  # Future: slack, webhooks, etc.
```

## CLI commands

### `lf maestro start`

Start the daemon in background:

```bash
$ lf maestro start
Maestro listening on ~/.lf/maestro.sock
```

### `lf maestro stop`

```bash
$ lf maestro stop
Maestro stopped
```

### `lf status`

Query running sessions:

```bash
$ lf status
TASK        WORKTREE              STATUS    STARTED
implement   .lf/worktrees/auth    running   2m ago
design      .lf/worktrees/api     waiting   5m ago

$ lf status --all  # across all repos
```

If maestro not running:
```bash
$ lf status
Maestro not running. Start with: lf maestro start
```

### Enhanced `-p` (print mode)

No new command. The existing `-p` flag gains tracking + notifications when maestro is running:

```bash
$ lf implement -p
# Registers with maestro (if running)
# Runs in batch mode
# Notifies on completion/error
```

The session:
1. Registers with maestro (if available)
2. Runs Claude Code in batch mode
3. Updates status on completion/error
4. Emits notification

## Integration with existing commands

Interactive commands (`lf design`, `lf ship`, `lf run <task>`) gain optional maestro integration:

```python
def run_task(...):
    session = Session(...)

    # Try to register, don't fail if maestro not running
    maestro = connect_maestro()
    if maestro:
        maestro.register(session)

    try:
        # ... run the task ...
        if maestro:
            maestro.update(session.id, SessionStatus.COMPLETED)
    except Exception as e:
        if maestro:
            maestro.update(session.id, SessionStatus.ERROR)
        raise
    finally:
        if maestro:
            maestro.unregister(session.id)
```

## File layout

```
~/.lf/
  maestro.sock        # Unix socket
  maestro.json        # Persisted state for recovery
  maestro.pid         # PID file for daemon
  config.yaml         # Global config (notification adapters, etc.)
```

## Constraints

- **macOS only** for now (Unix socket, osascript)
- **Claude Code backend** only for now (but adapter pattern allows others)
- **No webhook/cron triggers** in this version (future work)
- **Simple recovery**: if maestro crashes, sessions marked stale on restart

## Done when

```bash
# Start maestro
$ lf maestro start
Maestro listening on ~/.lf/maestro.sock

# In terminal 1: start interactive session
$ cd .lf/worktrees/auth
$ lf design
# ... session registers with maestro ...

# In terminal 2: check status
$ lf status
TASK     WORKTREE                STATUS    STARTED
design   .lf/worktrees/auth      running   1m ago

# In terminal 3: launch batch task (registers with maestro)
$ cd .lf/worktrees/api
$ lf implement -p &
# runs in background

$ lf status
TASK        WORKTREE                STATUS    STARTED
design      .lf/worktrees/auth      running   2m ago
implement   .lf/worktrees/api       running   10s ago

# When batch task completes:
# → macOS notification appears
# → lf status shows "completed"
```

## Open questions

1. **Waiting status**: How does maestro know when Claude Code is waiting for input? May need Claude Code hooks integration, or polling stdout for permission prompts.

2. **Session cleanup**: When does a completed session disappear from `lf status`? After N minutes? On next `lf status` query? Manual `lf status --clear`?

3. **Background agent output**: Where does stdout go for `lf bg`? Log file in worktree? Viewable via `lf logs <session_id>`?
