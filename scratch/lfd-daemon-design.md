# lfd Daemon Design Review

Should lfd be a daemon? If so, how should it work?

## What lfd Does Today

lfd is a background service providing:

1. **Session tracking** — logs step runs (start/end) for visibility
2. **Agent orchestration** — manages loop/watch/cron triggers
3. **Event pub/sub** — broadcasts worktree, session, agent events to subscribers (Concerto)
4. **PR polling** — periodic checks for PR status changes
5. **Worktree status** — tracks git worktree state across repos

Communication: Unix socket (`~/.lf/lfd.sock`) + HTTP (`localhost:8765`)
Storage: SQLite (`~/.lf/lfd.db`) in WAL mode

---

## Fundamental Question: Is a Daemon Right?

### What Requires a Daemon

| Capability | Needs Daemon? | Why |
|------------|---------------|-----|
| Loop mode (continuous) | Yes | Must persist across terminal sessions |
| Watch mode (file changes) | Maybe | launchd `WatchPaths` could do this |
| Cron mode (scheduled) | Maybe | launchd `StartCalendarInterval` could do this |
| Session tracking | No | Could log to file, query on-demand |
| Event broadcast | Yes | Real-time updates to Concerto |
| PR polling | Maybe | Could be on-demand or launchd-triggered |

### Alternative Architectures

**Option A: Pure launchd (no custom daemon)**

```xml
<!-- Watch mode via launchd -->
<key>WatchPaths</key>
<array><string>/path/to/repo/src</string></array>

<!-- Cron mode via launchd -->
<key>StartCalendarInterval</key>
<dict><key>Hour</key><integer>9</integer></dict>
```

Pros:
- No daemon to crash/manage
- OS handles restart, scheduling
- Lower resource usage

Cons:
- One plist per agent (management complexity)
- No real-time events to Concerto
- No unified status view
- Can't do complex trigger logic (e.g., "only if main changed")

**Option B: On-demand CLI (no daemon)**

```bash
lf loop ship src/  # runs in foreground, Ctrl-C to stop
```

Pros:
- Simple mental model
- No daemon lifecycle issues
- Works like `npm run dev`

Cons:
- Must keep terminal open
- No persistence across logout
- Multiple agents = multiple terminals

**Option C: Hybrid (minimal daemon + launchd triggers)**

Daemon only handles:
- Event pub/sub for Concerto
- Unified status queries
- Complex trigger evaluation

launchd handles:
- Actually invoking `lf flow` commands
- Restart on crash
- Schedule/watch triggers

**Option D: Keep current architecture, improve robustness**

Fix the sharp edges but keep the daemon approach.

### Recommendation

**Option D** — The daemon architecture is appropriate because:

1. **Real-time events** — Concerto needs live updates; polling is inferior
2. **Unified state** — Single source of truth for all agents/sessions
3. **Complex triggers** — Watch mode needs "main branch changed" logic, not just "file changed"
4. **Cross-agent coordination** — Future: rate limiting, resource sharing

The problems are implementation details, not architecture flaws.

---

## Enhancement 1: Process Lifecycle

### Current Problems

1. Port 8765 can be held by orphan processes
2. `lfd install` has race conditions with KeepAlive
3. No graceful shutdown — just SIGTERM/SIGKILL
4. PID file not used for stale detection

### Design Options

**1a. Trust launchd entirely**

Let launchd manage everything. On install:
1. Bootout existing service
2. Kill anything on port 8765 (verify it's lfd first)
3. Bootstrap new service

Pros: Simple, aligns with launchd model
Cons: Still need orphan cleanup

**1b. PID file with flock**

Use file locking for mutual exclusion:

```python
import fcntl

def acquire_lock():
    lock_file = open(PID_PATH, 'w')
    try:
        fcntl.flock(lock_file, fcntl.LOCK_EX | fcntl.LOCK_NB)
        lock_file.write(str(os.getpid()))
        lock_file.flush()
        return lock_file  # Keep open to hold lock
    except BlockingIOError:
        # Another process has the lock
        return None
```

Pros: Race-free, no orphan problem (lock released on crash)
Cons: Adds complexity, flock behavior varies across NFS

**1c. Health check with self-healing**

On startup:
1. Check if port 8765 responds to `/status`
2. If healthy, exit (daemon already running)
3. If unhealthy or timeout, kill and start fresh

```python
def ensure_healthy():
    try:
        r = requests.get('http://127.0.0.1:8765/status', timeout=2)
        if r.ok:
            return True  # Already running
    except:
        pass
    kill_orphan_lfd()
    return False  # Need to start
```

Pros: Self-healing, simple logic
Cons: Brief window of unavailability

### Recommendation

Combine **1a + 1c**: Trust launchd but add health check. On `lfd install`:

1. Hit `/status` — if healthy and version matches, done
2. Otherwise: bootout, kill orphans, bootstrap

---

## Enhancement 2: Graceful Shutdown

### Current Problems

1. SIGTERM just kills; no cleanup
2. Active connections dropped
3. SQLite WAL not checkpointed
4. No drain period for in-flight requests

### Design Options

**2a. Signal handler with timeout**

```python
async def shutdown(timeout=10):
    # Stop accepting new connections
    server.close()

    # Wait for in-flight requests
    await asyncio.wait_for(
        asyncio.gather(*pending_tasks),
        timeout=timeout
    )

    # Checkpoint WAL
    conn.execute("PRAGMA wal_checkpoint(TRUNCATE)")

    # Clean exit
    sys.exit(0)
```

**2b. Kubernetes-style prestop hook**

launchd doesn't support prestop, but we can fake it:

```xml
<key>ExitTimeOut</key>
<integer>30</integer>  <!-- Give us 30s before SIGKILL -->
```

Combined with a SIGTERM handler that does cleanup.

**2c. Two-phase shutdown**

1. SIGTERM → stop accepting, drain
2. SIGTERM again (or timeout) → force exit

### Recommendation

**2a** with **2b** configuration. Simple signal handler + adequate ExitTimeOut.

---

## Enhancement 3: Port Binding

### Current Problems

1. No SO_REUSEADDR — must wait for TIME_WAIT
2. Startup fails silently when port in use
3. No feedback on why startup failed

### Design Options

**3a. SO_REUSEADDR on HTTP server**

uvicorn supports this via config:

```python
config = uvicorn.Config(app, host="127.0.0.1", port=8765)
config.socket_options = [(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)]
```

**3b. Dynamic port with discovery**

Write port to file, clients read it:

```python
# Server
server = await asyncio.start_server(handler, '127.0.0.1', 0)
port = server.sockets[0].getsockname()[1]
Path("~/.lf/lfd.port").write_text(str(port))

# Client
port = int(Path("~/.lf/lfd.port").read_text())
```

Pros: No conflicts
Cons: More moving parts, breaks hardcoded port assumptions

**3c. Port conflict detection with clear error**

```python
def check_port():
    if not is_port_available(8765):
        pid = get_pid_on_port(8765)
        cmd = get_process_command(pid)
        raise StartupError(
            f"Port 8765 in use by PID {pid}: {cmd}\n"
            f"Run: kill {pid}"
        )
```

### Recommendation

**3a** (SO_REUSEADDR) + **3c** (clear errors). Dynamic ports add complexity without clear benefit.

---

## Enhancement 4: Schema Migrations

### Current State

Your migration system is solid:
- ISO timestamp versions (sortable, no collisions)
- Idempotent migrations
- Consolidation workflow
- `schema_migrations` tracking table

### Comparison to fastmigrate

| Feature | lfd current | fastmigrate |
|---------|-------------|-------------|
| Version tracking | `_meta` table + `schema_migrations` | `_meta.version` int |
| File naming | `m_YYYY_MM_DD_desc.py` | `NNNN-desc.{sql,py,sh}` |
| Idempotency | Manual (check before alter) | Manual |
| Rollback | None (by design) | None |
| Consolidation | Yes (collapse script) | No |

**fastmigrate doesn't offer anything lfd doesn't already have.** Your timestamp-based versioning is actually better for parallel development.

### Potential Improvements

**4a. Version compatibility checking**

Before running, verify the daemon code is compatible with the DB schema:

```python
MINIMUM_SCHEMA = "2026-01-20T00:00:00"
MAXIMUM_SCHEMA = "2026-02-01T00:00:00"

def check_compatibility(db_version):
    if db_version < MINIMUM_SCHEMA:
        raise IncompatibleError("Database too old. Run migrations.")
    if db_version > MAXIMUM_SCHEMA:
        raise IncompatibleError("Database too new. Update lfd.")
```

**4b. Backup before migration**

```python
def backup_before_migration(db_path):
    backup_path = db_path.with_suffix(f".db.backup.{datetime.now().isoformat()}")
    shutil.copy(db_path, backup_path)
    # Keep last 3 backups
    cleanup_old_backups(db_path.parent, keep=3)
```

**4c. WAL checkpoint before migration**

Ensure all changes are in main DB file before schema changes:

```python
def run_migrations(conn):
    conn.execute("PRAGMA wal_checkpoint(TRUNCATE)")
    # Then run migrations
```

### Recommendation

Add **4a** (compatibility check) and **4c** (WAL checkpoint). Backups (**4b**) are nice but add complexity for a local dev tool.

---

## Enhancement 5: Health Monitoring

### Current Problems

1. No way to know if daemon is healthy vs just running
2. Concerto shows "connected" but daemon might be degraded
3. No metrics or diagnostics

### Design Options

**5a. Rich health endpoint**

```json
GET /health

{
  "status": "healthy",
  "uptime_seconds": 3600,
  "version": "0.6.11",
  "schema_version": "2026-01-24T...",
  "checks": {
    "database": "ok",
    "socket": "ok",
    "disk_space": "ok"
  },
  "stats": {
    "active_sessions": 2,
    "agents_running": 1,
    "events_broadcast": 1523
  }
}
```

**5b. Structured logging**

JSON logs for easier parsing:

```json
{"ts": "2026-01-24T19:00:00", "level": "INFO", "event": "agent.started", "agent_id": "abc123"}
```

**5c. Prometheus metrics**

```
# HELP lfd_agents_total Total agents by status
lfd_agents_total{status="running"} 2
lfd_agents_total{status="idle"} 5
```

Overkill for a local tool, but useful if we ever want observability.

### Recommendation

**5a** — Rich health endpoint is high value, low effort. Helps debugging and Concerto can show status.

---

## Enhancement 6: Startup Reliability

### Current Problems

1. launchd may mark as "thrashing" if exit too fast
2. No startup delay/backoff
3. Errors not visible (only in log file)

### Design Options

**6a. Startup validation before serving**

```python
async def startup():
    # Validate everything before accepting connections
    assert_database_accessible()
    assert_schema_compatible()
    assert_socket_path_writable()

    # Only then start serving
    await start_servers()
```

**6b. ThrottleInterval in plist**

```xml
<key>ThrottleInterval</key>
<integer>10</integer>  <!-- Don't restart faster than every 10s -->
```

Prevents thrashing detection while still allowing restarts.

**6c. Startup notification to launchd**

launchd can wait for "ready" signal before considering service started:

```xml
<key>LaunchOnlyOnce</key>
<true/>  <!-- Don't auto-restart; we handle it -->
```

Or use machservice for proper readiness signaling (complex).

### Recommendation

**6a** + **6b** — Validate before serving, and add ThrottleInterval to plist.

---

## Summary of Recommendations

| Enhancement | Recommendation | Effort |
|-------------|----------------|--------|
| Process lifecycle | Health check + orphan cleanup | Medium |
| Graceful shutdown | SIGTERM handler + ExitTimeOut | Low |
| Port binding | SO_REUSEADDR + clear errors | Low |
| Schema migrations | Compatibility check + WAL checkpoint | Low |
| Health monitoring | Rich /health endpoint | Low |
| Startup reliability | Validation + ThrottleInterval | Low |

### Implementation Order

1. **Port binding** (SO_REUSEADDR) — fixes immediate pain
2. **Graceful shutdown** — prevents data loss
3. **Health endpoint** — helps debugging
4. **Process lifecycle** — comprehensive fix for orphan issues
5. **Startup reliability** — polish
6. **Schema compatibility** — future-proofing

---

## Decisions

1. **Protocol versioning** — Yes, version it. Daemon includes version in responses, clients can check compatibility.

2. **Orphan cleanup** — Always. No opt-in flag needed; `lfd install` always cleans up orphan processes.

3. **Health checks** — Passive only. `/health` endpoint for debugging. Concerto's connection attempt is the de facto health check.

4. **Single instance** — Enforce exactly one lfd per machine. Fail fast if another is running.
