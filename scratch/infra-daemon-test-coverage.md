# Session Handler Tests & Postgres Parity

## Problem

Wave 04 shipped trigger loop tests, wave CRUD handler tests, and token counting assertions. Two gaps remain: session handler coverage and Postgres parity.

The wave item describes `create_session_handler` and `get_session_handler` as "standard JSON handlers." This is wrong for `create_session_handler`. Wave CRUD handlers are thin wrappers over store operations. Session creation orchestrates prompt assembly (reads step/direction/area files from disk), harness construction (spawns agent processes), and async runtime management. Testing it at the handler level means testing `SessionManager::create_session` with HTTP ceremony on top.

`get_session_handler` and `delete_session_handler` (for terminal sessions) are genuinely thin — they read/update the store and map errors.

## Approach

### 1. Extract `test_http_state()` to shared test helper

`test_http_state()` lives in `waves.rs`'s `#[cfg(test)]` module. Session handler tests need it too. Extract to a `#[cfg(test)]` helper module at `http/routes/test_helpers.rs` (or a `test_support` submodule), re-export from both test modules.

Add a variant that accepts a custom `SessionManager`:

```rust
async fn test_http_state() -> HttpState { ... }
async fn test_http_state_with_sessions(sessions: SessionManager) -> HttpState { ... }
```

The second variant lets session tests inject a `SessionManager` with a fake harness factory.

### 2. Session handler tests — store-seeded path

Test `get_session_handler` and `delete_session_handler` by seeding `Session` records directly in the store. No harness, no prompt assembly.

| Test | Setup | Assert |
|------|-------|--------|
| `get_session_returns_dto` | Seed session via `store.create_session()` | Handler returns `SessionDto` with matching fields |
| `get_session_not_found` | No seeding | Handler returns 404 |
| `delete_terminal_session_is_idempotent` | Seed session with `status: Ended` | Handler returns the session unchanged |

### 3. `create_session_handler` — skip handler-level test

`create_session_handler` is 12 lines of delegation. The interesting logic is in `SessionManager::create_session`, which requires:

- A real directory on disk (validated by `validate_repo_root`)
- Step files resolvable by `gather_context` (reads `.lf/steps/`, installed steps, etc.)
- A harness factory that doesn't spawn real agents (requires `with_create_harness`, currently private)
- Tolerance for async task spawning (harness event bridge, startup task)

Testing this at the handler level would require: making `with_create_harness` pub(crate), writing a no-op `Harness` impl, setting up a tempdir with step files, and tolerating background task spawning. The ROI is low — the handler's own logic is just `CreateSessionRequest → CreateSessionParams` mapping and `SessionManagerError → StatusCode` mapping, both trivial.

The error mapping is already implicitly covered by `map_session_error`, which is a pure match statement testable in isolation if needed.

### 4. Postgres parity test

A single test function that exercises core store operations against whichever backend `DATABASE_URL` points to. Gated with `#[ignore]` so `cargo test` skips it by default; CI runs it with `cargo test -- --ignored` when a Postgres instance is available.

```rust
#[tokio::test]
#[ignore] // requires DATABASE_URL
async fn postgres_store_parity() {
    let url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let store = open_store(&StorageConfig::postgres(url)).await.expect("connect");
    run_store_parity_suite(&store).await;
}
```

`run_store_parity_suite` runs the same operations already tested in `sqlite_store_basic_suite`: wave CRUD, wave run lifecycle, stimulus CRUD, session CRUD, and agent lifecycle. Factor the existing SQLite test's body into a shared function both tests call.

Location: `rust/loopflow/src/lfd/store/mod.rs` alongside existing store tests, or `tests/store_parity.rs` if the existing file is too large.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Test `create_session_handler` with full repo + fake harness | Thorough coverage of the HTTP boundary | Low ROI: handler is pure delegation, requires complex fixture (tempdir, step files, pub(crate) visibility change), and tests `SessionManager` more than the handler |
| Duplicate `test_http_state()` in sessions.rs | Simpler, no module extraction | Violates one-source-of-truth; the function is 40 lines with 12 dependencies |
| Skip Postgres parity entirely | Simpler | Leaves a real gap — SQLite and Postgres have different SQL semantics (e.g., `RETURNING`, type coercion) and the store has explicit backend dispatch |

## Key decisions

**Store-seeded session tests over handler-orchestrated tests.** The handler is a thin wrapper. The store-seeded approach tests the handler's DTO mapping and error handling without requiring the full `SessionManager::create_session` machinery. This matches how the existing `backfill_lagged_events` test works — it seeds the store directly.

**Extract `test_http_state()` rather than duplicate.** Two copies of a 40-line function with 12 type imports is a maintenance liability. A shared `#[cfg(test)]` module keeps it DRY.

**`#[ignore]` for Postgres, not `#[cfg]`.** `#[ignore]` lets developers run the test locally with `--ignored` without feature flags. CI controls whether it runs via test flags, not compile-time configuration.

## Scope

- In scope: extract `test_http_state()`, 3 session handler tests, refactor SQLite store test into shared suite, add `#[ignore]` Postgres parity test
- Out of scope: `create_session_handler` handler test, `stream_session_events_handler`, `send_session_input_handler`, CI Postgres infrastructure

## Done when

```bash
cargo test -p loopflow session_handler  # 3 new tests pass
cargo test -p loopflow sqlite_store     # existing tests still pass via shared suite
DATABASE_URL=postgres://... cargo test -p loopflow postgres_store_parity -- --ignored  # passes against local Postgres
```
