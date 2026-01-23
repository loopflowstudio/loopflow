# LFD Reliability: Remaining Phases

Continue hardening watch, loop, and cron modes after foundation work is complete.

## Context

Phases 0-2 are implemented (schema isolation, state lifecycle, subprocess timeout). These phases focus on the trigger logic and operational hardening.

---

## Phase 3: Watch Mode

Reliable file-change detection.

1. **Pure trigger function**
   ```python
   def should_trigger_watch(
       watch_paths: list[str],
       last_sha: str | None,
       current_sha: str,
       changed_files: list[str]
   ) -> bool:
   ```

2. **Unit tests**
   - No previous SHA → trigger
   - SHA same → no trigger
   - SHA different, no matching paths → no trigger
   - SHA different, matching paths → trigger

3. **Handle git failures**
   - `git fetch` fails → log, don't update SHA, don't trigger
   - Never silently succeed when git failed

4. **Debounce**
   - Track `last_trigger_time` per agent
   - Don't re-trigger if < 5 min ago

---

## Phase 4: Cron Mode

Triggers exactly when expected.

1. **Pure trigger function**
   ```python
   def should_trigger_cron(
       cron_expr: str,
       last_run_ended: datetime | None,
       now: datetime
   ) -> bool:
   ```

2. **Fix 24-hour grace period**
   - Current: prevents triggers for 24 hours after any run
   - New: use `croniter.get_next(last_run_ended)`, compare to `now`

3. **Unit tests**
   - `*/5 * * * *`, last run 6 min ago → trigger
   - Same, last run 2 min ago → no trigger
   - No previous run → trigger

---

## Phase 5: Loop Mode Hardening

Recovers from transient failures.

1. **Retry with backoff**
   - On failure: wait 30s, retry up to 3 times
   - 3rd failure: status=ERROR, stop loop

2. **Track consecutive failures**
   - New field: `consecutive_failures` on Agent
   - Reset to 0 on success
   - Circuit breaker: > 5 failures → pause, notify

3. **Health endpoint**
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

## Priority

| Phase | Effort | Risk Reduction |
|-------|--------|----------------|
| 3 | S | Medium |
| 4 | S | Medium |
| 5 | M | Medium |
| 6 | M | High |

Phases 3 and 4 can be done in parallel. Phase 5 depends on 3+4. Phase 6 is independent.

---

## Quick Wins

Do anytime:

- Replace remaining `except Exception: pass` with `except Exception as e: logger.warning(...)`
- Add `--dry-run` to `check_watch` and `check_cron`
- Add `lfd doctor`: check schema version, orphaned runs, dead PIDs
