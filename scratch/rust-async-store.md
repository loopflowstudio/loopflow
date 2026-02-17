# 02: Async Store Boundary

Remove the sync/async bridging layer. Routes and executor call async store methods directly.

## What exists after this

HTTP routes `await` store methods. No `spawn_blocking` for database operations. PostgresStore drops its embedded `tokio::Runtime` and uses the server's runtime natively. SqliteStore uses `spawn_blocking` internally (rusqlite is sync), but callers don't see it.

## Current state

Three layers of bridging:

1. **Route → store**: `run_store(store, |s| s.method())` in `lfd/http/mod.rs:122-131` wraps every store call in `spawn_blocking`
2. **PostgresStore**: holds a `tokio::runtime::Runtime` field, calls `self.runtime.block_on(future)` to bridge async tokio-postgres into the sync `RunStore` trait
3. **Capability traits**: `WaveStateStore`/`ExecutionStore`/`StoreAdmin` are async but delegate to sync `RunStore` via `as_run_store()`

~45 route handlers use `run_store()`. System routes (`system.rs`) use raw `spawn_blocking` for some store calls.

## Approach

### Step 1 — Make backends implement capability traits directly

PostgresStore implements `WaveStateStore`/`ExecutionStore`/`StoreAdmin` as native async (tokio-postgres is already async). Drop the embedded runtime and `block_on` helper.

SqliteStore wraps each method in `spawn_blocking` internally. Callers see async; the blocking is contained.

Remove the `RunStore` → capability trait delegation in `Store`. `Store` dispatches to backend-specific async implementations.

### Step 2 — Migrate HTTP routes

Replace `run_store(store, |s| s.method())` with `store.method().await`. Change `HttpState` to hold `Arc<Store>` instead of `SharedStore` (`Arc<dyn RunStore>`).

System routes that use raw `spawn_blocking` for store calls migrate the same way.

### Step 3 — Remove bridging infrastructure

- Delete `run_store()` from `lfd/http/mod.rs`
- Delete `SharedStore` type alias (or redefine as `Arc<Store>`)
- Delete `RunStore` trait
- Delete `PostgresStore::runtime` field and `block_on()` method
- Delete `Store::as_run_store()` and `Store::into_shared()`

## Key files

| File | Lines | What changes |
|------|-------|-------------|
| `lfd/store/mod.rs` | ~860 | Remove `RunStore` trait, `Store::as_run_store()`, delegation impls |
| `lfd/store/postgres.rs` | ~970 | Drop embedded runtime; implement capability traits natively |
| `lfd/store/sqlite.rs` | ~824 | Implement capability traits with internal `spawn_blocking` |
| `lfd/http/mod.rs` | ~132 | Delete `run_store()` helper |
| `lfd/http/routes/*.rs` | ~1500 | Replace `run_store(...)` calls with direct `.await` |
| `lfd/executor.rs` | ~4043 | Replace `store.method()` sync calls with `.await` |

## Risks

- **Executor is sync-heavy**: The executor calls store methods synchronously in many places. These need to become `.await` calls, which means executor methods that call the store must become async. This is a large diff but mechanically straightforward.
- **Test changes**: Store tests currently call sync methods. They'll need a tokio test runtime or switch to the async interface.

## Done when

- No `run_store()` calls in HTTP routes
- No `spawn_blocking` for database operations in route handlers
- PostgresStore has no embedded runtime
- `RunStore` trait deleted
- All existing tests pass with the async interface
