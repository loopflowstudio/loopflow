# lfd — Loopflow Daemon

Background service for agent orchestration.

## Usage

```bash
# One-shot: create + configure + run
lfd loop swift-falcon --area src/

# Incremental
lfd create swift-falcon
lfd area swift-falcon src/
lfd goal swift-falcon "fix lint errors"
lfd loop swift-falcon

# Other stimuli
lfd watch swift-falcon --area src/           # run on origin/main changes
lfd cron swift-falcon "0 9 * * *" --area .   # run daily at 9am
```

See `docs/lfd.md` for the full CLI reference.

## Agents

Agents are persistent configurations with:
- **name**: User-visible identifier (e.g., "swift-falcon")
- **area**: Working directories (required to run)
- **goal**: What to accomplish (inline text or preset name)
- **flow**: Which steps to execute (default: ship)
- **stimulus**: When to run (run/loop/watch/cron)

Create with minimal config, configure incrementally, run when ready.

## Stimulus Modes

| Mode | Trigger | Use Case |
|------|---------|----------|
| `run` | Manual, once | One-off task |
| `loop` | After each iteration | Continuous improvement |
| `watch` | origin/main changes | React to upstream |
| `cron` | Schedule | Daily maintenance |

## Database

SQLite at `~/.lf/lfd.db` (WAL mode).

### agents table

| Column | Type | Description |
|--------|------|-------------|
| id | TEXT PK | UUID |
| name | TEXT | User-visible name |
| area | TEXT | JSON array of paths (nullable until configured) |
| goal | TEXT | JSON array of goals (nullable) |
| flow | TEXT | Flow name (default: ship) |
| repo | TEXT | Repository path |
| stimulus_kind | TEXT | run, loop, watch, cron |
| stimulus_cron | TEXT | Cron expression (for cron stimulus) |
| status | TEXT | idle, running, waiting, error |
| iteration | INTEGER | Iteration count |
| ... | | |

## Protocol

JSON-over-newline on Unix socket at `~/.lf/lfd.sock`.

See protocol.py for Request/Response/Event dataclasses.

## HTTP API

```
POST   /agents           {name?, area?, goal?, flow?}     → create
PATCH  /agents/:id       {area?, goal?, flow?}            → update
GET    /agents                                            → list
GET    /agents/:id                                        → show
DELETE /agents/:id                                        → delete
POST   /agents/:id/run   {stimulus, cron?, path?}         → run
POST   /agents/:id/stop                                   → stop
```

- Create accepts minimal body (even empty → generates name)
- Update accepts any subset of fields
- Validation happens on run, not create/update
- CLI commands like `lfd area` map to `PATCH` with that field

## Debugging

### Logs

```bash
tail -f ~/.lf/logs/lfd.log                    # watch live
grep "swift-falcon" ~/.lf/logs/lfd.log        # filter by agent
grep ERROR ~/.lf/logs/lfd.log                 # find errors
```

### Database

```bash
sqlite3 ~/.lf/lfd.db ".schema"
sqlite3 ~/.lf/lfd.db "SELECT id, name, status, area FROM agents"
```

### Circuit Breaker

When an agent fails >= 5 times consecutively:
- Status becomes ERROR
- Won't restart until reset

Reset:
```bash
sqlite3 ~/.lf/lfd.db "UPDATE agents SET consecutive_failures = 0, status = 'idle' WHERE name = 'swift-falcon'"
```
