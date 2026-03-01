# Review: Daemon Test Coverage

## What was implemented

Test coverage for the four daemon trigger loops (cron, loop_ticker, recovery, watch) and wave CRUD HTTP handlers. One production refactor: extracting `paths_match_areas` from `watch.rs`. Token counting assertions upgraded from `>= 1` placeholders to exact values.

**Files changed:**
- `rust/loopflow/src/lfd/triggers/cron.rs` — 5 tests for `should_activate_cron`
- `rust/loopflow/src/lfd/triggers/loop_ticker.rs` — 5 tests for `should_pause_for_max_iterations`
- `rust/loopflow/src/lfd/triggers/recovery.rs` — 3 tests for `get_stuck_agents` at the store level
- `rust/loopflow/src/lfd/triggers/watch.rs` — `paths_match_areas` extraction + 5 tests
- `rust/loopflow/src/lfd/http/routes/waves.rs` — `test_http_state()` + `init_git_repo()` helpers, `make_wave()` factory, 5 new CRUD handler tests
- `rust/loopflow/tests/token_tests.rs` — exact assertions replacing placeholders
- `scratch/infra-daemon-test-coverage.md` — design doc

## Key choices

1. **Test decision logic, not polling plumbing.** Each trigger loop has a pure decision function that determines whether to activate. Tests target those functions directly rather than trying to test the `spawn_*_poller` async loops.

2. **`paths_match_areas` extraction.** The only production refactor. Path matching was inlined in `check_watch_stimulus` alongside git I/O. Extracting it makes path matching testable without git repos. Clean separation of concerns — the function was already a conceptual unit.

3. **Store-level test for recovery.** `get_stuck_agents` is the real correctness risk (the SQL query). Testing the full `recover_stuck_runs` cascade would require constructing a `WaveExecutor`, which needs real agent infrastructure.

4. **Cron tests use relative expressions.** Since `should_activate_cron` calls `Utc::now()` internally, tests construct cron expressions relative to the current time rather than injecting a clock parameter into production code.

5. **Wave handler tests follow `chords.rs` pattern.** Direct handler calls with `test_http_state()` and `State()` wrappers, matching the established convention.

## How it fits together

The trigger modules (`cron`, `loop_ticker`, `recovery`, `watch`) each run on a timer in lfd. Each has a core predicate that decides whether to activate a wave. The tests verify these predicates with edge cases (never triggered, just triggered, at limit, below limit, etc.).

Wave CRUD handler tests verify the HTTP handlers that external clients use to manage waves — create, get, list with filters, update, delete, and stimulus management.

## Risks and bottlenecks

- **Cron timing sensitivity.** Tests construct cron expressions relative to `Utc::now()`. If a test runs exactly at a minute boundary, assertions about "just triggered" could theoretically flake. The cron library uses second-precision scheduling, and the tests use minute-level expressions with a 1-minute offset, so this is unlikely in practice.

- **TempDir drop in `test_http_state()` / `test_store()`.** The `TempDir` is dropped before the function returns, relying on Unix fd semantics to keep the SQLite database accessible. This is the established pattern in `chords.rs` and other test modules — works reliably on macOS and Linux.

- **Pre-existing `config_tests` failure.** `load_config_or_default_returns_defaults` fails because the local `.lf/config.yaml` has `yolo: true`. Not related to this branch. The test depends on environment state rather than using isolation — a known issue.

## What's not included

- **Postgres parity** — needs CI infrastructure.
- **Session handler tests** — SSE streaming makes direct handler testing awkward.
- **Orchestration handlers** (`run_wave_handler`, `stop_wave_handler`) — spawn real agents, need executor infrastructure.
- **`queue_reconcile.rs`** — 40-line dispatch loop, low correctness risk.
- **`summary_refresh.rs`** — debounce with `Instant`, low value.
