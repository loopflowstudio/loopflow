# 04: Daemon Test Coverage

## Problem

Six trigger loops control when waves activate and how stuck agents are cleaned up. Zero tests. The HTTP handlers have real business logic (CRUD, lifecycle, authorization routing) with no HTTP-level tests — only the auth middleware is covered. Token counting assertions are placeholder `>= 1` checks that pass for any positive output. A Postgres backend with ~2000 lines has no tests at all.

This is the biggest gap between "architecturally sound" and "production-grade" — the code works, but nothing proves it. Regressions in trigger logic or handler behavior are invisible until they break a user's daemon.

Wave goal: "Every background trigger loop has at least one test" and "Test count for trigger loops (target: >0)."

## Approach

Test the decision logic, not the plumbing. Each trigger has a core predicate that determines whether it fires. Test those predicates directly. For handlers, use the established pattern (real SQLite store, handler calls with axum extractors) — no mock stores, no factory traits.

### 1. Trigger loop unit tests

**`cron.rs` — `should_activate_cron`** (pure function, highest value)
- Expression due now → true
- Expression not due for hours → false
- Unparseable expression → false
- Grace window: expression was due 23h ago → true (daemon was down, picks up on restart)
- Grace window boundary: expression was due 25h ago → false (outside 24h window)

**`loop_ticker.rs` — `should_pause_for_max_iterations`** (pure function)
- `max_iterations = None` → false (no limit)
- `max_iterations = 0` → false (zero means unlimited)
- At limit → true
- Below limit → false
- `cycle_start_iteration > 0` → correct offset math

**`watch.rs` — extract path-matching logic**
The area-matching logic is inlined inside `check_watch_stimulus` alongside git I/O. Extract `fn paths_match_areas(areas: &[String], paths: &[&str]) -> bool` and test:
- Empty areas → always matches
- Single area prefix matches a path → true
- No paths match any area → false
- Nested path matches parent area → true

**`recovery.rs`** — the "is stuck" decision lives inside the SQL query (`get_stuck_agents`), not in recovery.rs itself. The recovery loop is pure plumbing. Test at the store level: insert an agent with `started_at` 5 hours ago, verify `get_stuck_agents(4 * 3600)` returns it. Insert one 3 hours old, verify it doesn't.

**`queue_reconcile.rs`** — delegates entirely to `lfd::queue`. No logic to test here. Skip.

**`token_refresh.rs`** — already has 6 tests. No work needed.

**`ci_failure.rs`** — already has 3 tests. No work needed.

### 2. HTTP handler integration tests

Use the established pattern from `chords.rs`/`repos.rs`: construct `HttpState` with a temp SQLite store, call handlers directly with axum extractors.

**Wave handlers** (highest value — most complex):
- Create wave → get wave → update wave → verify fields changed
- Delete wave → get returns error
- List waves with repo filter
- `resolve_wave_id` works with both name and ID

**Session handlers**:
- Create session → get session → verify state
- Delete session → verify status change

**Webhook handler**:
- Empty `webhook_secret` → 503 (verified safe, but a test documents the guarantee)

### 3. Postgres parity test

A single test function that runs the same store operations against both SQLite and Postgres. Operations: create wave, list waves, update wave, delete wave, stimulus CRUD, activation log CRUD.

Gate on `DATABASE_URL` env var — skip when Postgres isn't available. CI can add a Postgres service container later.

### 4. Token counting assertions

Replace the `>= 1` placeholders with exact expected values. Run `count_tokens("hello")` locally, record the result, use `assert_eq!`. Same for the 30-char repeated string.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Mock the store for trigger tests | Isolates from DB, but tests become mock-wiring assertions | CLAUDE.md says "mock side effects, assert on results, not mock calls." The existing tests use real SQLite — follow the pattern |
| Full HTTP integration tests via reqwest | Tests the full middleware stack including auth | Existing handler tests call handlers directly with extractors. Middleware (auth, body limits) is already tested separately in `http/mod.rs`. Follow the pattern |
| Snapshot/golden tests for handler responses | Catches unexpected response shape changes | Brittle. Asserting on specific fields is clearer about what matters |
| Docker Postgres in every CI run | Real parity testing | Adds CI complexity and time. Conditional skip with `DATABASE_URL` lets us add it later without blocking |

## Key decisions

**Extract `paths_match_areas` from `watch.rs`.** The path-matching logic is currently tangled with git I/O. Extracting it creates a pure function we can test without a git repo. This is a small refactor that pays for itself immediately — the alternative is spinning up a git fixture with real commits, which is fragile and slow.

**Skip `queue_reconcile.rs` and `summary_refresh.rs`.** Both are thin dispatchers — `queue_reconcile` delegates to `lfd::queue`, `summary_refresh` delegates to summary generation. Testing the dispatcher adds no confidence. Test the underlying logic instead, if it lacks coverage.

**`get_stuck_agents` test goes in the store test suite, not recovery.rs.** The decision logic (the 4-hour threshold, the SQL predicate) lives in the store. Testing it at the recovery level would require constructing an executor and other plumbing that adds nothing.

**Handler tests call handlers directly, not via HTTP.** This is the established pattern. It tests the business logic without rediscovering that axum routing works.

## Scope

- In scope: Tests for `cron.rs`, `loop_ticker.rs`, `watch.rs` (with extraction), `recovery.rs` (store-level), wave/session/webhook handler tests, token counting fix, Postgres parity test
- Out of scope: New CI infrastructure for Postgres, testing `common.rs` (pure plumbing), testing `summary_refresh.rs` (debounce logic with `Instant` is hard to test and low value), refactoring trigger code beyond the `paths_match_areas` extraction

## Done when

```bash
cargo test --all  # all new tests pass
cargo test -p loopflow should_activate_cron  # cron predicate tests
cargo test -p loopflow should_pause  # loop ticker tests
cargo test -p loopflow paths_match  # watch area-matching tests
cargo test -p loopflow stuck_agents  # store-level recovery test
cargo test -p loopflow wave_handler  # HTTP handler tests
cargo test -p loopflow token_counting  # exact assertions, not >= 1
```

Test count for trigger loops goes from 0 to >0 (wave metric). Every trigger that has decision logic gets at least one test. Handlers that do CRUD get round-trip tests.
