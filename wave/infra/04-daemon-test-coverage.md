# 04: Daemon Test Coverage

**Finish line:** Every lfd trigger loop (cron, loop_ticker, recovery, watch) has inline tests. Wave CRUD and stimulus HTTP handlers have handler-level tests. Token counting assertions use exact values.

## Scope

**Trigger loop tests.** Inline `#[cfg(test)]` modules in each file. Real SQLite stores, no mocks. Test both decision functions (pure) and check functions (store reads/writes).

| Module | What to test |
|--------|--------------|
| `cron.rs` | `should_activate_cron` with never-triggered, just-triggered, past-due, invalid expression. `check_cron_waves` with seeded cron wave. |
| `loop_ticker.rs` | `should_pause_for_max_iterations` at boundary and with max=0. `tick_loop_waves` skipping active/pending waves, activating idle loopable wave. |
| `recovery.rs` | `recover_stuck_runs` — agent over 4h threshold marked Failed (cascading to run and wave), agent under threshold untouched, multiple stuck agents independent. Insert records with past timestamps for time manipulation. |
| `watch.rs` | `check_watch_stimulus` — matching area triggers, non-matching area skips, no new commits skips, first-ever check bootstraps SHA. Use `TestRepo` from `loopflow-test-support` (local bare remote, real git2 codepath). |

**Skip `token_refresh.rs`** (already has 6 tests with FakeTokenRefresher) and **`queue_reconcile.rs`** (40-line dispatch loop, testing it just verifies iteration).

**Wave CRUD handler tests.** Call handlers directly with `State(state)`, `Json(payload)`, `Path(id)` extractors. Cover `create_wave_handler`, `get_wave_handler`, `list_waves_handler`, `update_wave_handler`, `delete_wave_handler`, `add_stimulus_handler`, `remove_stimulus_handler`.

**Skip orchestration handlers** (`run_wave_handler`, `stop_wave_handler`, `land_wave_handler`, `combine_wave_handler`, `next_wave_handler`) — they depend on WaveExecutor which spawns real agents. Testing at handler level would be mock wiring.

**Token counting.** Replace `>= 1` assertions in `token_tests.rs` with exact values for `"hello"` and `"a".repeat(30)` using cl100k_base.

**Out of scope:** Postgres parity (needs CI infrastructure), session handler tests, queue_reconcile tests.

## Risks

- **watch.rs tests are the most complex** — TestRepo setup with commits, fetches, and area filtering. Most likely place for implementation friction.
- **recovery.rs needs past timestamps** — insert records with `created_at` in the past rather than mocking `Utc::now()`.
