# LFD Reliability: Foundation

Schema isolation, state lifecycle, and subprocess hardening.

## Completed

### Phase 0: Database Isolation ✓

1. **Schema version enforcement**
   - `SCHEMA_VERSION` constant in `db.py`
   - `_meta` table stores current version
   - `SchemaMismatchError` raised on mismatch (unless reset enabled)
   - `LF_DB_RESET=1` env var or `reset_on_mismatch=True` resets DB

2. **Tests**: `test_db_schema_*` in `tests/test_lfd.py`

### Phase 1: State Lifecycle ✓

1. **Cleanup sweep**
   - `cleanup_stale_runs()` in `flow_run.py`
   - Runs on daemon startup (`server.py:start()`)
   - Runs every 30s in periodic check
   - Handles: orphaned runs, dead agent PIDs, deleted agents

2. **mark_run_failed()** - always succeeds

3. **Tests**: `test_cleanup_stale_runs_*`, `test_mark_run_failed`

### Phase 2: Subprocess Lifecycle ✓

1. **Step timeout**
   - `StepTimeoutError` in `runner.py`
   - `DEFAULT_STEP_TIMEOUT = 30 * 60` (30 minutes)
   - `_run_collector_step(timeout=...)` parameter
   - `_kill_process_tree()` for cleanup
   - Handled in fork/join groups

2. **Exit code propagation** - already working

3. **Tests**: `test_step_timeout_*`, `test_kill_process_tree_*`

---

## Next Steps

See `roadmap/lfd-reliability.md` for Phases 3-6:
- Phase 3: Watch Mode (pure triggers, debounce)
- Phase 4: Cron Mode (fix grace period)
- Phase 5: Loop Mode (retry, circuit breaker)
- Phase 6: Transaction Boundaries
