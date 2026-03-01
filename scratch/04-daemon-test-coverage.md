# 04: Daemon Test Coverage

## Problem

Five trigger loops control when waves activate and how stuck agents recover. They have zero test coverage. The wave and session HTTP handlers — the primary API surface — have no handler-level tests. A broken cron schedule or a botched recovery cleanup would ship undetected.

Tests here aren't about coverage metrics. These are the control loops that decide "should this wave run?" and "is this agent dead?" — getting those wrong burns money or loses work.

## Approach

Inline `#[cfg(test)]` modules in each trigger and handler file. Real SQLite stores (no mocks), following the established pattern. Test the decisions, not the plumbing.

### Trigger loop tests

Each trigger has two layers: a **decision function** (pure or near-pure) and a **check function** (reads/writes store). Test both.

| Module | Decision function | Check function | Test focus |
|--------|-------------------|----------------|------------|
| `cron.rs` | `should_activate_cron` | `check_cron_waves` | Cron expression evaluation, last-triggered tracking, activation on schedule |
| `loop_ticker.rs` | `should_pause_for_max_iterations` | `tick_loop_waves` | Max iteration pause, skip-when-active, skip-when-pending |
| `recovery.rs` | — | `recover_stuck_runs` | Stuck agent detection, cascading status updates (agent → run → wave) |
| `watch.rs` | area path matching | `check_watch_stimulus` | SHA diff detection, area filtering, bootstrap (first-seen SHA) |
| `token_refresh.rs` | — | — | Already has comprehensive tests. Skip. |
| `queue_reconcile.rs` | — | — | Thin dispatch loop. Skip — the real logic is in `reconcile_wave_queue`. |

**cron.rs tests:**
- `should_activate_cron("0 * * * *", None)` → true (never triggered, overdue)
- `should_activate_cron("0 * * * *", Some(now - 30s))` → false (just triggered)
- `should_activate_cron("0 * * * *", Some(now - 2h))` → true (past due)
- Invalid cron expression → false (no panic)
- `check_cron_waves` with a seeded cron wave → creates pending activation

**loop_ticker.rs tests:**
- `should_pause_for_max_iterations` at boundary (iteration == cycle_start + max - 1 → false, +max → true)
- `should_pause_for_max_iterations` with max=0 → false (unlimited)
- `tick_loop_waves` skips wave with active session
- `tick_loop_waves` skips wave with pending activation
- `tick_loop_waves` activates idle loopable wave

**recovery.rs tests:**
- Agent running 5 hours → marked Failed, run marked Failed, wave marked Failed
- Agent running 3 hours → untouched (under 4h threshold)
- Multiple stuck agents → all recovered independently

**watch.rs tests:**
- New commit on main with matching area → triggers
- New commit on main with non-matching area → no trigger
- No new commits → no trigger
- First-ever check (no last SHA) → bootstraps SHA, no trigger

Use `TestRepo` from `loopflow-test-support` for watch tests. Push commits to the bare remote, then let `check_watch_stimulus` fetch from it. Tests the real git logic without external network calls.

### HTTP handler tests

Follow the existing pattern: call handlers directly with `State(state)`, `Json(payload)`, `Path(id)` extractors. No HTTP client needed.

Focus on **waves.rs** — 18 handlers, zero tests, the biggest API surface gap.

| Handler | Test cases |
|---------|------------|
| `create_wave_handler` | Creates wave, returns DTO with correct fields |
| `get_wave_handler` | Get by ID, get by name, not found → 404 |
| `list_waves_handler` | Empty list, multiple waves, pagination |
| `update_wave_handler` | Update fields, partial update preserves others |
| `delete_wave_handler` | Delete existing, delete nonexistent → 404 |
| `add_stimulus_handler` | Add stimulus to wave |
| `remove_stimulus_handler` | Remove stimulus |

Skip `run_wave`, `stop_wave`, `land_wave`, `combine_wave`, `next_wave` — these orchestrate external processes (git, agents) and need end-to-end tests, not unit tests. Testing them as handler calls would just be testing mock wiring.

### Token counting

Replace the `>= 1` assertions in `token_tests.rs` with exact values. Run `count_tokens` with cl100k_base to determine the actual token counts for `"hello"` and `"a".repeat(30)`, then assert equality.

### Postgres parity

Skip for this PR. The wave item says "requires a Postgres fixture in CI or a conditional skip" — that's CI infrastructure work that doesn't belong in a test coverage PR. File as a separate wave item if needed.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Mock the store with a trait object | Decouples tests from SQLite | Style guide says "never reshape production code for tests." Store isn't trait-based. Real SQLite is fast enough. |
| Test trigger loops via spawn + sleep | Tests the full async loop with real timing | Flaky, slow, tests timing not logic. Test the decision/check functions directly instead. |
| HTTP tests via tower::oneshot with router | Tests middleware + routing + handler together | Existing tests call handlers directly. Follow the pattern. Middleware (auth) is tested separately. |
| Test all 18 wave handlers | Completeness | Handlers that orchestrate external processes (run, stop, land) need e2e tests, not handler-level tests. Testing them here would require mocking the executor, which tests wiring not behavior. |
| Postgres parity in this PR | The wave item asks for it | Needs CI infrastructure (Postgres container). Separate concern from test coverage. |

## Key decisions

**Skip `token_refresh.rs` and `queue_reconcile.rs`.** The wave item lists both, but token_refresh already has comprehensive tests (FakeTokenRefresher, multi-provider scenarios, timeout handling). queue_reconcile is a 20-line dispatch loop — testing it would just verify that it iterates over waves, which is plumbing.

**Inline tests, not integration test files.** The decision functions (`should_activate_cron`, `should_pause_for_max_iterations`) are private. Inline `#[cfg(test)]` modules can access them. This also follows the pattern in `token_refresh.rs`, `hooks.rs`, `repos.rs`.

**Real git repos for watch tests.** `TestRepo` gives us a local bare remote. No network calls, but tests the actual git2 codepath including fetch, SHA comparison, and tree diff.

**CRUD handlers only for waves.** The orchestration handlers (run, stop, land, combine, next) depend on WaveExecutor which spawns real agents and manipulates git. Testing those at the handler level would require faking the executor — that's exactly the "mock wiring" the style guide warns against.

## Scope

- In scope: cron, loop_ticker, recovery, watch trigger tests. Wave CRUD + stimulus handler tests. Token counting fix.
- Out of scope: Postgres parity, session handler tests, orchestration handler tests (run/stop/land/combine/next), queue_reconcile tests.

## Done when

```bash
cargo test trigger    # new trigger tests pass
cargo test waves      # new wave handler tests pass
cargo test token      # token_tests assertions are exact values
cargo test --all      # no regressions
cargo clippy -- -D warnings  # clean
```
