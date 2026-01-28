# LFD Reliability

Hardening for watch, loop, and cron agent modes.

## Status

| Phase | Description | Status |
|-------|-------------|--------|
| 0-2 | Foundation (schema, lifecycle, timeout) | ✓ Done |
| 3 | Watch mode trigger logic | ✓ Done (debounce TODO) |
| 4 | Cron mode trigger logic | ✓ Done |
| 5 | Loop mode resilience | ✓ Done (health endpoint TODO) |
| 6 | Transaction boundaries | TODO |

**Test coverage**: 107 tests in `test_lfd.py`

---

## Phase 3: Watch Mode ✓ (mostly complete)

Reliable file-change detection.

1. **Pure trigger function** ✓
   ```python
   def should_trigger_watch(
       watch_paths: list[str],
       last_sha: str | None,
       current_sha: str,
       changed_files: list[str]
   ) -> bool:
   ```

2. **Unit tests** ✓ (11 tests)
   - No previous SHA → no trigger (first run records baseline)
   - SHA same → no trigger
   - SHA different, no matching paths → no trigger
   - SHA different, matching paths → trigger
   - Exact file match, glob patterns, multiple paths
   - Trailing slash handling, partial path rejection

3. **Handle git failures** ✓
   - `git fetch` fails → return False, don't update SHA
   - `git diff` fails → update SHA, return False

4. **Debounce** (TODO)
   - Track `last_trigger_time` per agent
   - Don't re-trigger if < 5 min ago

---

## Phase 4: Cron Mode ✓ (mostly complete)

Triggers exactly when expected.

1. **Pure trigger function** ✓ (already existed)
   ```python
   def should_trigger_cron(
       cron_expr: str,
       last_run_ended: datetime | None,
       now: datetime
   ) -> bool:
   ```

2. **Grace period** ✓ (works correctly)
   - Skips triggers beyond grace period (default 24h)
   - First run triggers if within grace

3. **Unit tests** ✓ (10 tests)
   - `*/5 * * * *`, last run 6 min ago → trigger
   - Same, last run 2 min ago → no trigger
   - No previous run → trigger
   - Stale beyond grace → no trigger
   - Daily/hourly schedules

---

## Phase 5: Loop Mode Hardening ✓ (mostly complete)

Recovers from transient failures.

1. **Retry with backoff** ✓
   - On failure: wait 30s, retry up to 3 times
   - 3rd failure: status=ERROR, stop loop
   - Configurable: `MAX_RETRIES=3`, `RETRY_BACKOFF_SECONDS=30`

2. **Track consecutive failures** ✓
   - New field: `consecutive_failures` on Agent
   - Reset to 0 on success
   - Circuit breaker: >= 5 failures → pause, emit `agent.circuit_breaker` event
   - Migration: `m_2025_01_23_consecutive_failures.py`

3. **Comprehensive logging** ✓
   - All agent operations logged to `~/.lf/logs/lfd.log`
   - Separate loggers: `lfd.agent`, `lfd.worker`, `lfd.trigger`
   - Full stack traces on exceptions

4. **Health endpoint** (TODO)
   - `GET /health` → `{status: "ok", active: N, failed: M}`

---

## Phase 6: Transaction Boundaries

Database operations are atomic.

1. **Transaction context manager**
   ```python
   with db_transaction() as conn:
       update_run(conn, run_id, status="RUNNING")
       update_agent(conn, agent_id, iteration=n)
   # commits on success, rolls back on exception
   ```

2. **Audit multi-statement operations**
   - `run_iteration()`: worktree + run save + step updates
   - Wrap in transaction

3. **Connection pooling**
   - Single connection per daemon process
   - Or `check_same_thread=False` with lock

---

## Remaining Work

| Item | Phase | Effort |
|------|-------|--------|
| Watch debounce (`last_trigger_time`) | 3 | S |
| Health endpoint (`GET /health`) | 5 | S |
| Transaction boundaries | 6 | M |
| `lfd doctor` command | — | S |

Phase 6 is independent and can be done anytime.
