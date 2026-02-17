# 02: Async Store Boundary

## Problem

lfd still crosses sync/async boundaries three times for every database call: HTTP routes use `run_store(...)` + `spawn_blocking`, `PostgresStore` owns its own runtime and `block_on`s async queries, and `Store` async traits delegate back into sync `RunStore`.

This costs latency, obscures ownership of blocking work, and makes new storage features error-prone. Users feel it as slower API responses under load; maintainers feel it as repetitive plumbing and hard-to-reason concurrency.

Why now: Stage 01 explicitly kept `RunStore` as temporary compatibility glue. Stage 02 is where we delete it.

## Approach

Adopt a single async storage boundary: every caller `await`s store methods; blocking is isolated inside backend implementations only.

1. **Make `Store` the shared handle**
   - Replace `SharedStore = Arc<dyn RunStore>` with `SharedStore = Arc<Store>`.
   - Delete `RunStore`, `Store::into_shared()`, and `Store::as_run_store()`.
   - Keep capability traits (`WaveStateStore`, `ExecutionStore`, `StoreAdmin`) as the public async contract.

2. **Implement backends as native async capability stores**
   - **PostgresStore**: remove embedded `tokio::runtime::Runtime` field and `block_on()` helper; implement capability traits with direct async `tokio-postgres` calls.
   - **SqliteStore**: keep rusqlite synchronous internally, but move blocking into backend methods via `tokio::task::spawn_blocking`.
     - Change `conn: Mutex<Connection>` to `conn: Arc<Mutex<Connection>>` so closures passed to `spawn_blocking` can own cloned state.
     - Ensure locking happens inside blocking closures only.

3. **Migrate runtime call sites to async store API**
   - HTTP: replace all `run_store(...)` uses with direct `store.method(...).await`.
   - Remove DB-related `spawn_blocking` in routes (keep non-DB blocking operations like git/worktree/process calls).
   - System/WS/hooks helpers move to async store methods and drop `&dyn RunStore` helper signatures.
   - Executor + triggers + scheduler paths switch sync store calls to async `await` calls, including helper functions like `create_wave_run_with_id`.

4. **Delete bridging infrastructure in the same PR**
   - No compatibility layer. No dual sync+async paths.
   - Any compile error from removed `RunStore` is treated as a migration to-do, not a place to add shim code.

5. **Update tests to async-first**
   - Convert store suites from sync helpers (`&dyn RunStore`) to async helpers over capability traits.
   - Use `#[tokio::test]` where store methods are invoked.
   - Keep backend coverage for both sqlite and postgres paths.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Keep `RunStore` and only remove `run_store()` from HTTP | Smaller diff now | Preserves core bridge complexity; Postgres still runs nested runtime; wave principle “Store operations are async end-to-end” stays unfulfilled |
| Introduce a second trait (`AsyncRunStore`) and migrate gradually | Lower immediate risk | Creates long-lived dual interfaces and duplicated methods; violates “Each stage deletes what it replaces” |
| Replace rusqlite with fully async DB stack now | Cleaner long-term architecture | Too large for Stage 02 scope; couples async-boundary cleanup with backend rewrite |

## Key decisions

- **Decision: one async path, no fallback sync path.**
  - Following wave principles: **“Store operations are async end-to-end”** and **“Each stage deletes what it replaces — no deferred cleanup.”**
- **Decision: keep sqlite, isolate blocking internally.**
  - Caller ergonomics stay uniform (`await` everywhere) while preserving existing sqlite implementation.
- **Decision: migrate executor and trigger paths in this stage, not later.**
  - Wild success: wave loops remain responsive under concurrent activity because DB calls no longer block async workers unpredictably.
  - Wild failure to avoid: partial migration leaves mixed sync/async access patterns, reintroducing hidden blocking and making deadlocks/latency regressions hard to diagnose.

## Scope

- In scope:
  - Remove `RunStore` and `run_store()`
  - Async capability trait impls for PostgresStore and SqliteStore
  - `SharedStore` migration to `Arc<Store>`
  - HTTP + executor + triggers + scheduler migration to async store calls
  - Store/executor tests updated for async interface
- Out of scope:
  - Shared SQL catalog work (Stage 03)
  - Prompt pipeline work (Stage 04)
  - Executor module decomposition
  - Behavior changes to wave semantics/scheduling policy

## Done when

```bash
# No sync bridge remains
rg -n "trait RunStore|run_store\(|as_run_store\(|into_shared\(" rust/loopflow/src/lfd

# Postgres store has no embedded runtime bridge
rg -n "runtime:\s*tokio::runtime::Runtime|fn block_on<" rust/loopflow/src/lfd/store/postgres.rs

# HTTP DB access is direct async (no DB spawn_blocking wrappers)
rg -n "spawn_blocking\(move \|.*store\." rust/loopflow/src/lfd/http/routes rust/loopflow/src/lfd/http/mod.rs

# Build + tests
cargo test -p loopflow lfd::store
cargo test -p loopflow lfd::http
cargo test -p loopflow lfd::executor
cargo clippy -p loopflow -- -D warnings
```

And the server runs with both sqlite and postgres configs using only async store calls at call sites.
