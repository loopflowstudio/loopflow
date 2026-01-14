# laozi

Unix socket daemon (`lfd`) for agent orchestration, replacing SQLite polling with real-time IPC.

## Review

**Verdict:** Ready to ship

All style issues from the previous review have been fixed. Imports are at file tops. Functions that accessed `maestro.markdown` and `maestro.worktree` have been moved into `lfd/agents.py` to keep `lfd` self-contained.

## Design notes

Architecture: `lfd` is a Unix socket daemon (like postgres) that owns agent lifecycle, trigger evaluation, and session tracking. CLI (`lf`) and Maestro (Swift app) are clients. SQLite persists state across daemon restarts; socket is for real-time IPC.

launchd label: `com.loopflow.lfd`

CLI structure: All agent commands moved to `lfd` binary (`lfd new`, `lfd list`, `lfd start`, `lfd stop`, etc.). Old `lf agent` and `lf ops maestro` commands removed.

Dependencies removed: `fastapi`, `uvicorn` (FastAPI web UI replaced by Maestro app).

Files deleted: `cli/agent.py`, `cli/maestro.py`.
