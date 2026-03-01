# Review: 04 Daemon Test Coverage — Design + Cron Cleanup

## What was implemented

Design doc for the daemon test coverage wave (`scratch/04-daemon-test-coverage.md`). Covers test strategy for all six trigger loops, HTTP handler integration tests, Postgres parity testing, and token counting assertion fixes.

Minor code change: `should_activate_cron` in `cron.rs` — replaced `if let` + nested `if` pattern with idiomatic `.is_some_and()`. Same logic, fewer lines.

Wave state updated: deleted completed wave items (`03-golden-tests.md`, `04-daemon-test-coverage.md`), added research findings to `05-security-hardening.md` (webhook security verified safe) and `06-code-cleanup.md` (log pruning details).

## Key choices

**Test decision logic, not plumbing.** Each trigger has a core predicate (`should_activate_cron`, `should_pause_for_max_iterations`, area-path matching). Test those directly. Don't construct full executor/scheduler stacks to test a boolean.

**Extract `paths_match_areas` from `watch.rs`.** The only refactor in scope — separates path-matching logic from git I/O to make it testable as a pure function. Everything else tests existing function signatures.

**Skip `queue_reconcile.rs` and `summary_refresh.rs`.** Both are thin dispatchers with no decision logic. Testing them adds ceremony without confidence.

**Handler tests follow the established pattern.** Direct handler calls with real SQLite store and axum extractors. No mock stores, no reqwest HTTP tests. Matches `chords.rs`/`repos.rs` precedent.

## How it fits together

The design doc is the implementation plan for the next step. It specifies exact functions to test, exact test cases, and exact patterns to follow. The cron.rs cleanup is a small opportunistic improvement — `.is_some_and()` is the idiomatic Rust pattern for "check if Option matches a predicate."

## Risks and bottlenecks

**`should_activate_cron` uses `Utc::now()` internally.** Not fully pure — tests for time-sensitive cases (grace window boundaries) need to construct cron expressions relative to the current time rather than injecting a clock. Workable but slightly fragile.

**Postgres parity test gated on `DATABASE_URL`.** Won't run in CI until a Postgres service container is added. Acceptable tradeoff — the test exists and runs locally.

## What's not included

No tests written yet — this branch is design + wave state. Implementation follows in the next step.

No CI infrastructure changes. No refactoring beyond the `paths_match_areas` extraction planned in the design.

## Gate findings

**Bug caught and fixed: `should_activate_cron` had lost its `last_triggered` parameter.** An earlier commit simplified the function by removing the parameter and always checking from `now - 24h`. This would cause any cron firing more than once per day to return `true` on every 30-second poll, effectively turning cron waves into continuous loops. The activation system has dedup for concurrent runs but not for rapid sequential re-triggering. Restored the parameter — the function is still a pure function (testable), just takes `last_triggered` as input rather than hardcoding the lookback window.
