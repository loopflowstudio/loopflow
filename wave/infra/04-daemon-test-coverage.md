# 04: Daemon Test Coverage

**Finish line:** Session handler integration tests exist. Postgres parity test runs when `DATABASE_URL` is set.

## Scope

**Session handler tests.** `create_session_handler` and `get_session_handler` are standard JSON handlers — test with the same `test_http_state()` + direct handler call pattern used for wave CRUD. Skip `stream_session_events_handler` (SSE streaming body) and `send_session_input_handler`. The existing `backfill_lagged_events` test in `sessions.rs` covers event replay logic — the gap is handler-level CRUD coverage.

`test_http_state()` is defined inside `waves.rs`'s test module. Either extract to a shared test helper or duplicate in `sessions.rs`.

**Postgres parity test.** Same store operations against SQLite and Postgres, gated on `DATABASE_URL` env var. Low priority until Postgres is available in CI.
