# Review: Daemon Test Coverage Design

## What was implemented

Design doc for adding test coverage to lfd's five trigger loops (cron, loop_ticker, recovery, watch) and wave CRUD HTTP handlers. Moved the wave item from `wave/infra/` to `scratch/` and elaborated it into a concrete test plan.

## Key choices

- **Inline `#[cfg(test)]` modules** over integration test files — decision functions are private, and this matches `token_refresh.rs`, `hooks.rs`, `repos.rs`.
- **Real SQLite stores** over mocked stores — follows style guide ("never reshape production code for tests"), and SQLite is fast enough.
- **Skip token_refresh and queue_reconcile** — token_refresh already has 6 tests with FakeTokenRefresher; queue_reconcile is a 40-line loop that delegates to `reconcile_wave_queue`.
- **CRUD handlers only** for waves.rs — orchestration handlers (run/stop/land/combine/next) depend on WaveExecutor which spawns real agents. Testing those here would be mock wiring.
- **TestRepo from loopflow-test-support** for watch tests — local bare remote, real git2 codepath, no network calls.
- **Postgres parity deferred** — needs CI infrastructure, separate concern.

## Verification

All references in the design doc were verified against the codebase:
- All 6 trigger files exist with the named functions
- All 7 CRUD/stimulus handlers exist in waves.rs
- `token_tests.rs` has the `>= 1` assertions as described
- `TestRepo` exists with the expected API (new, create_file, commit, push, etc.)
- Existing inline test patterns confirmed in token_refresh.rs, hooks.rs, repos.rs

Two naming corrections applied:
- Orchestration handlers: added `_handler` suffix to match actual function names
- queue_reconcile.rs: corrected "20-line" to "40-line"

## Risks and bottlenecks

- **watch.rs tests are the most complex** — they need TestRepo setup with commits, fetches, and area filtering. Most likely place for implementation friction.
- **recovery.rs tests need time manipulation** — testing "agent running 5 hours" requires either mocking `Utc::now()` or inserting records with timestamps in the past. The design doc doesn't specify the approach.

## What's not included

- Postgres parity tests (CI infrastructure work)
- Session handler tests
- Orchestration handler tests (run/stop/land/combine/next)
- queue_reconcile tests
- No code changes — this is design only
