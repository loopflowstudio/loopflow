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

## Debugging

### Logs

All agent operations are logged to `~/.lf/logs/lfd.log`:

```bash
tail -f ~/.lf/logs/lfd.log                    # watch live
grep "agent-id" ~/.lf/logs/lfd.log            # filter by agent
grep ERROR ~/.lf/logs/lfd.log                 # find errors
grep TRIGGERED ~/.lf/logs/lfd.log             # see trigger events
```

Log levels:
- `DEBUG`: detailed trigger checks, attempt counts
- `INFO`: agent start/stop, iteration success, trigger events
- `WARNING`: retries, failed git operations
- `ERROR`: exceptions, circuit breaker trips

### Database

Inspect the SQLite database directly:

```bash
sqlite3 ~/.lf/lfd.db ".schema"                # see tables
sqlite3 ~/.lf/lfd.db "SELECT * FROM agents"   # list agents
sqlite3 ~/.lf/lfd.db "SELECT id, status, consecutive_failures FROM agents"
```

Reset the database (use with caution):

```bash
LF_DB_RESET=1 lfd status                      # resets on schema mismatch
rm ~/.lf/lfd.db                               # nuclear option
```

### Manual Testing

Test trigger logic without starting agents:

```bash
# Check watch mode
cd /path/to/repo
git fetch origin main
git diff --name-only <last-sha> origin/main -- src/

# Check cron logic
python -c "
from loopflow.lfd.agent import should_trigger_cron
from datetime import datetime, timedelta
print(should_trigger_cron('*/5 * * * *', datetime.now() - timedelta(minutes=10)))
"
```

### Circuit Breaker

When an agent fails repeatedly (>= 5 times), it trips the circuit breaker:
- Status becomes ERROR
- `agent.circuit_breaker` event emitted
- Won't restart until `consecutive_failures` is reset

Reset a tripped agent:

```bash
sqlite3 ~/.lf/lfd.db "UPDATE agents SET consecutive_failures = 0, status = 'idle' WHERE id = '<agent-id>'"
```

### Retry Behavior

On iteration failure:
1. Wait 30 seconds
2. Retry up to 3 times
3. If all retries fail: increment `consecutive_failures`, mark ERROR
