# 04: Daemon Test Coverage

**Finish line:** Every lfd trigger loop has at least one test. HTTP handler integration tests exist for wave and session routes.

## Scope

**Trigger loop tests.** `cron.rs`, `watch.rs`, `loop_ticker.rs`, `recovery.rs`, `token_refresh.rs`, `queue_reconcile.rs` — high-value behaviors with zero test coverage. These control when waves activate and how stuck agents are cleaned up. Write unit tests that exercise the core logic of each trigger (mock the store, verify the decisions).

`cron.rs` context: `should_activate_cron` was simplified — the unused `last_triggered` parameter was removed and the function is now a pure predicate: parse the cron expression, check if any scheduled time in the past 24 hours is due. The 24h grace window means a daemon that was down picks up missed cron jobs on restart. Test the core predicate (expressions that are due vs. not due) and the grace window behavior.

**HTTP handler integration tests.** The route handlers in `http/routes/` have no HTTP-level tests. The auth middleware is tested but the business logic in wave, session, and webhook handlers is not. Write tests that construct an axum test server with a real SQLite store and exercise the request/response cycle.

**Postgres backend test.** `store/postgres.rs` is 1938 lines with no tests. At minimum, add a parity test that runs the same store operations against both SQLite and Postgres (requires a Postgres fixture in CI or a conditional skip).

**Token counting tests.** Replace the three `>= 1` assertions in `token_tests.rs` with known-good values for specific inputs.
