# 04: Daemon Test Coverage

**Finish line:** Every background trigger loop has at least one test. HTTP handler integration tests exist for wave and session routes. Token counting assertions use exact expected values.

## Scope

Test decision logic, not plumbing. Each trigger has a core predicate — test those directly. Handlers follow the established pattern (real SQLite store, direct handler calls with axum extractors).

**Trigger loop unit tests:**
- `cron.rs` — `should_activate_cron`: due now, not due, unparseable, grace window (23h ago → true, 25h ago → false). Note: uses `Utc::now()` internally — construct cron expressions relative to current time rather than injecting a clock.
- `loop_ticker.rs` — `should_pause_for_max_iterations`: None/0 → false, at limit → true, below → false, offset math with `cycle_start_iteration > 0`.
- `watch.rs` — extract `fn paths_match_areas(areas: &[String], paths: &[&str]) -> bool` from `check_watch_stimulus`. Test: empty areas → matches, prefix match, no match, nested path matches parent area.
- `recovery.rs` — test `get_stuck_agents` at the store level: insert agent started 5h ago, verify returned by `get_stuck_agents(4 * 3600)`. Insert 3h old, verify excluded.
- Skip `queue_reconcile.rs` (thin dispatcher) and `summary_refresh.rs` (debounce with `Instant`, low value).

**HTTP handler integration tests:**
- Wave handlers: create → get → update → verify, delete → get returns error, list with repo filter, `resolve_wave_id` with name and ID.
- Session handlers: create → get → verify state, delete → verify status change.
- Webhook handler: empty `webhook_secret` → 503.

**Postgres parity test:** Same store operations against SQLite and Postgres. Gate on `DATABASE_URL` env var — skip when unavailable.

**Token counting:** Replace `>= 1` placeholders with `assert_eq!` on exact expected values.

## Key decisions

- Extract `paths_match_areas` from `watch.rs` — only refactor in scope. Separates path-matching from git I/O for pure-function testing.
- `get_stuck_agents` test goes in the store test suite, not `recovery.rs`. The decision logic lives in the SQL query.
- Handler tests call handlers directly, not via HTTP. Follows `chords.rs`/`repos.rs` precedent.
- No mock stores, no factory traits. Real SQLite, assert on results.

## Context from prior review

Bug caught and fixed on this branch: `should_activate_cron` had lost its `last_triggered` parameter. An earlier commit simplified the function by removing the parameter and always checking from `now - 24h`, which would cause any cron firing more than once per day to re-trigger on every 30-second poll. Restored the parameter — function is still pure, just takes `last_triggered` as input.
