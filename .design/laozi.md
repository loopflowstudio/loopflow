# laozi

Unix socket daemon (`lfd`) for agent orchestration, replacing SQLite polling with real-time IPC.

## Review

**Verdict:** Ready to ship

All tests passing. README updated to reflect new `lfd` commands.

## Design notes

Architecture: `lfd` is a Unix socket daemon that owns agent lifecycle, trigger evaluation, and session tracking. CLI (`lf`) and Maestro (Swift app) are clients. SQLite persists state across daemon restarts; socket is for real-time IPC.

launchd label: `com.loopflow.lfd`

### CLI structure

New `lfd` binary with commands:
- `lfd serve` - run daemon in foreground
- `lfd status` - show daemon and agent status (default command)
- `lfd install` / `lfd uninstall` - launchd plist management
- `lfd list` - list all agents
- `lfd start` / `lfd stop` - control agent runs
- `lfd show` / `lfd logs` - view agent details
- `lfd new` / `lfd edit` / `lfd rm` - manage agent definitions

Old `lf agent` and `lf ops maestro` commands removed.

### Dependencies

Removed: `fastapi`, `uvicorn` (FastAPI web UI replaced by Maestro app).

### Files

New: `src/loopflow/lfd/` module with:
- `__init__.py` - CLI entry point
- `server.py` - asyncio Unix socket server
- `client.py` - client for connecting to daemon
- `protocol.py` - JSON-over-newline protocol
- `models.py` - data structures (AgentSpec, AgentRun, Session)
- `db.py` - SQLite persistence
- `agents.py` - agent loading and spawning
- `triggers.py` - trigger evaluation
- `launchd.py` - plist management
- `process.py` - process utilities

Deleted: `cli/agent.py`, `cli/maestro.py`.
