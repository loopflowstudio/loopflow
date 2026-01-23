# lfd — Loopflow Daemon

Background service for session tracking and agent orchestration.

## Usage

```bash
lfd install
lfd loop ship src/
lfd subscribe ship src/ -p src/
lfd schedule ship . "0 9 * * *"
lfd status
```

See `docs/lfd.md` for the full CLI reference.

## Runs and Triggers

Runs are execution instances of a flow. Triggers (loop, subscription, schedule) spawn
runs and track their own status, iteration count, and PR limits.

Parent encoding is stored on each run as `loop:<id>`, `subscription:<id>`, or
`schedule:<id>` to keep the model portable.

## Database

SQLite at `~/.lf/lfd.db` (WAL mode).

### runs, loops, subscriptions, schedules

Runs record each execution. Triggers store configuration and status for background
spawning.

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

## Protocol

JSON-over-newline on Unix socket at `~/.lf/lfd.sock`.

See protocol.py for Request/Response/Event dataclasses.

## Fire-and-Forget Pattern

StepRun logging uses `_send_fire_and_forget()` — synchronous socket with
0.5s timeout, fails silently. This prevents lfd availability from blocking
task execution. If daemon is down, step runs aren't logged but tasks still run.

## Client Patterns

- Async client: `DaemonClient` for CLI/tests (connect, call, subscribe)
- Sync fire-and-forget: `log_step_run_start()`, `log_step_run_end()` for lf runner
