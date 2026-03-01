# 04: Daemon Test Coverage

## Problem

Four trigger loops (cron, loop_ticker, recovery, watch) run every 5-60 seconds in lfd with zero test coverage. Each has a pure decision function — `should_activate_cron`, `should_pause_for_max_iterations`, path matching in `check_watch_stimulus`, `get_stuck_agents` — that represents real correctness risk. Wave CRUD handlers handle all external state mutations but have no handler-level tests despite an established pattern in `chords.rs` and `repos.rs`. Token counting assertions use `>= 1` placeholders instead of exact values.

The infra wave's goal: "Every background trigger loop has at least one test."

## Approach

Test the decision logic, not the polling plumbing. Each trigger module gets an inline `#[cfg(test)]` module testing its core predicate. Wave handlers get a test module following the established `test_http_state()` + direct handler call pattern. One production refactor: extract `paths_match_areas` from `watch.rs` to make path matching testable without git I/O.

### Trigger loop tests

All in inline `#[cfg(test)]` modules. Real SQLite stores where store access is needed. No mocks.

**`cron.rs`** — Test `should_activate_cron` directly (it's already pure):
- `never_triggered_within_grace_period` — no last_triggered, cron expr that fires now → true
- `never_triggered_outside_grace_period` — no last_triggered, cron expr that only fired >24h ago → false
- `just_triggered` — last_triggered 1 minute ago, hourly cron → false
- `past_due` — last_triggered 2 hours ago, hourly cron → true
- `invalid_expression` — garbage string → false

Construct cron expressions relative to `Utc::now()` since the function calls `Utc::now()` internally. Use expressions like `"0 {min} {hour} * * * *"` formatted with the current time.

**`loop_ticker.rs`** — Test `should_pause_for_max_iterations` directly (pure function):
- `no_max_iterations` — max_iterations: None → false
- `max_zero` — max_iterations: Some(0) → false (max > 0 check fails)
- `below_limit` — iteration 2, cycle_start 0, max 5 → false
- `at_limit` — iteration 4, cycle_start 0, max 5 → true (cycle_iterations = 5 >= 5)
- `with_offset` — iteration 7, cycle_start 5, max 5 → false (cycle_iterations = 3)

Build minimal `Stimulus` and `Wave` structs with only the fields that matter.

**`recovery.rs`** — Test at the store level via `get_stuck_agents`:
- `agent_over_threshold_returned` — insert agent started 5h ago (via store `start_agent` with past timestamp), call `get_stuck_agents(4 * 3600)` → returned
- `agent_under_threshold_excluded` — insert agent started 3h ago → not returned
- `multiple_agents_independent` — insert one 5h old and one 3h old → only 5h old returned

Insert records with `created_at` timestamps in the past. No clock mocking needed — the SQL query compares against `now_unix() - older_than_secs`.

**`watch.rs`** — Extract `paths_match_areas` as a pure function, test it:
- `empty_areas_matches_everything` — areas: [] → true for any paths
- `prefix_match` — areas: ["src/api/"], paths: ["src/api/handler.rs"] → true
- `no_match` — areas: ["src/api/"], paths: ["docs/README.md"] → false
- `nested_path_matches_parent_area` — areas: ["src/"], paths: ["src/api/deep/file.rs"] → true
- `multiple_areas_any_match` — areas: ["src/", "docs/"], paths: ["docs/README.md"] → true

The extraction:
```rust
fn paths_match_areas(areas: &[String], paths: &[&Path]) -> bool {
    if areas.is_empty() {
        return true;
    }
    paths.iter().any(|path| {
        areas.iter().any(|area| path.starts_with(area))
    })
}
```

Then `check_watch_stimulus` calls `paths_match_areas(wave.area(), &changed_paths)` instead of inlining the logic.

### Wave CRUD handler tests

New `#[cfg(test)]` module at the bottom of `waves.rs`. Reuse the `test_http_state()` pattern from `chords.rs`.

Test sequence:
- `create_and_get` — create_wave_handler → get_wave_handler, verify fields match
- `list_with_filter` — create two waves in different repos, list with repo filter → only matching wave returned
- `update_fields` — create → update name/flow/direction → get → verify new values
- `delete_then_get` — create → delete → get returns NOT_FOUND
- `stimulus_add_remove` — create wave → add_stimulus_handler → list_stimuli_handler → verify → remove_stimulus_handler → list → empty

### Token counting

Replace placeholders in `token_tests.rs` with exact `assert_eq!` values. Run cl100k_base locally to determine:
- `count_tokens("hello")` → exact value (1 token — common word)
- `count_tokens("a".repeat(30))` → exact value (compute by running the test with a print)

### Out of scope

- Postgres parity (needs CI infrastructure)
- Session handler tests (SSE streaming makes direct handler testing awkward)
- `queue_reconcile.rs` (40-line dispatch loop)
- Orchestration handlers (`run_wave_handler`, `stop_wave_handler`, etc. — spawn real agents)
- `token_refresh.rs` (already has 6 tests)
- `summary_refresh.rs` (debounce with `Instant`, low value)

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Mock stores for trigger tests | Faster tests, but tests mock wiring not behavior | CLAUDE.md: "Mock side effects, but don't test mock wiring" |
| Full integration tests via HTTP client | Tests routing + serialization too | Over-tests plumbing; established pattern is direct handler calls |
| Inject clock into `should_activate_cron` | Makes function testable with fixed times | Function is already testable — construct cron expressions relative to now. Adding a clock parameter reshapes production code for tests. |
| Test `recover_stuck_runs` directly | Covers cascade logic (agent → run → wave status updates) | Needs WaveExecutor, which is hard to construct without spawning real agent infrastructure. Store-level `get_stuck_agents` test covers the real correctness risk — the SQL query. |

## Key decisions

1. **One production refactor: `paths_match_areas` extraction.** This is the only case where code needs to change for testability. The path matching is currently inlined in `check_watch_stimulus` which does git I/O. Extracting it is a clean separation of concerns, not test-driven reshaping.

2. **Store-level test for recovery, not function-level.** The decision logic in `recover_stuck_runs` is the SQL query in `get_stuck_agents`, not the Rust code. Testing the cascade (marking agent/run/wave as Failed) would require constructing a WaveExecutor, which is the kind of mock wiring the style guide prohibits.

3. **Cron tests use relative expressions.** Since `should_activate_cron` calls `Utc::now()` internally, tests construct cron expressions relative to the current time. This avoids injecting a clock parameter into production code.

4. **Token exact values determined empirically.** Run the tokenizer in a test, print the values, hardcode them. cl100k_base is deterministic.

## Scope

- **In scope:** 4 trigger module test suites, wave CRUD handler tests, `paths_match_areas` extraction, token counting exact values
- **Out of scope:** Postgres parity, session handlers, orchestration handlers, queue_reconcile, summary_refresh

## Done when

```bash
cargo test -p loopflow -- cron::tests        # cron decision tests pass
cargo test -p loopflow -- loop_ticker::tests  # iteration limit tests pass
cargo test -p loopflow -- recovery            # store-level stuck agent tests pass
cargo test -p loopflow -- watch::tests        # path matching tests pass
cargo test -p loopflow -- waves::tests        # handler CRUD tests pass
cargo test -- token_counting                  # exact token assertions pass
cargo test --all                              # no regressions
```

Wave goal advanced: "Every background trigger loop has at least one test" — all four trigger loops covered. "No silent data corruption path" — recovery cascade verified at store level.
