# lfd — Loopflow Daemon

Background service for session tracking and agent orchestration.

## Database

SQLite at `~/.lf/lfd.db` (WAL mode).

### sessions table
| Column | Type | Description |
|--------|------|-------------|
| id | TEXT PK | UUID |
| task | TEXT | Task name (design, implement, etc.) |
| repo | TEXT | Repository path |
| worktree | TEXT | Worktree path |
| status | TEXT | running, waiting, completed, error |
| started_at | TEXT | ISO8601 |
| ended_at | TEXT | ISO8601 or NULL |
| pid | INTEGER | Process ID |
| model | TEXT | claude-code, codex, etc. |
| run_mode | TEXT | auto or interactive |

### agent_runs table
| Column | Type | Description |
|--------|------|-------------|
| id | TEXT PK | UUID |
| agent_name | TEXT | Agent definition name |
| status | TEXT | idle, running, waiting, error, stopped |
| started_at | TEXT | ISO8601 |
| ended_at | TEXT | ISO8601 or NULL |
| pid | INTEGER | Process ID |
| worktree | TEXT | Current worktree path |
| iteration | INTEGER | Run count |
| error | TEXT | Error message or NULL |
| main_sha | TEXT | Main branch SHA at start |

## Protocol

JSON-over-newline on Unix socket at `~/.lf/lfd.sock`.

See protocol.py for Request/Response/Event dataclasses.

## Fire-and-Forget Pattern

Session logging uses `_send_fire_and_forget()` — synchronous socket with
0.5s timeout, fails silently. This prevents lfd availability from blocking
task execution. If daemon is down, sessions aren't logged but tasks still run.

## Client Patterns

- Async client: `DaemonClient` for CLI/tests (connect, call, subscribe)
- Sync fire-and-forget: `log_session_start()`, `log_session_end()` for lf runner
