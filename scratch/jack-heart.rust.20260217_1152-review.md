# Async Store Boundary — PR Review Guide

## What was implemented

Migrated lfd from a sync bridge pattern (`RunStore` trait + `run_store()` helper + `Store::as_run_store()`) to direct async store calls. Every call site now `await`s `Arc<Store>` methods directly. `SharedStore` is `Arc<Store>`.

Concrete changes:
- Removed `RunStore`, `run_store()`, `Store::as_run_store()`, `Store::into_shared()`
- Migrated all HTTP routes, executor, triggers, and tests to async-first patterns
- SqliteStore isolates blocking via `spawn_blocking` inside backend methods
- PostgresStore calls `tokio-postgres` directly; embedded runtime removed
- Added chat memory block store methods (list/upsert/delete) across all layers
- Updated wave/rust README and consolidated scratch docs

## Key choices

**One async path, no shim.** The sync bridge was removed entirely rather than deprecated. This means every call site was touched in one pass. The alternative — keeping `RunStore` as a compatibility layer — would have left two code paths to maintain with no benefit since all callers are async.

**`Arc<Store>` as concrete type.** `SharedStore` is a type alias for `Arc<Store>`, not `Arc<dyn Trait>`. This avoids dynamic dispatch overhead and keeps the store API discoverable through IDE navigation. The `Store` enum dispatches to backends internally.

**SQLite blocking contained in backend.** Rather than pushing `spawn_blocking` to callers, each SqliteStore method wraps its rusqlite access internally. This keeps the async boundary uniform — callers don't need to know which backend they're talking to.

## How it fits together

```
HTTP routes / executor / triggers
        │
        │ .await
        ▼
   Arc<Store>  (SharedStore)
        │
        ├── Store::Sqlite(SqliteStore)   → spawn_blocking → rusqlite
        └── Store::Postgres(PostgresStore) → tokio-postgres (native async)
```

Store exposes `pub async fn` methods. The `Store` enum matches on its backend variant and delegates. SqliteStore clones an `Arc<Mutex<Connection>>` into `spawn_blocking` closures. PostgresStore uses its `deadpool` connection pool directly.

## Risks and bottlenecks

- **SQLite write serialization**: Single `Mutex<Connection>` means heavy write bursts queue on the blocking pool. Not a problem at current scale but would surface under concurrent wave execution.
- **Broad migration surface**: ~30 files touched. Regression risk is mitigated by 466 passing tests, but less-traveled trigger paths (cron/watch activation with pending queues) have no integration test coverage.
- **`spawn_blocking` join failure mapping**: SQLite wraps `JoinError` as `StoreError::InvalidData`, which is semantically wrong (it's an infrastructure failure, not bad data). Fine for now but would confuse debugging if the blocking pool is saturated.

## What's not included

- **Shared SQL catalog** (Stage 03): Queries are still duplicated between `sqlite.rs` and `postgres.rs` (~45 queries each). Unifying them is the next stage.
- **Prompt pipeline changes** (Stage 04): Independent of store work.
- **Executor decomposition**: Out of scope per wave plan.
- **SQLite replacement**: Intentionally kept as the default backend.

## Validation

| Check | Result |
|-------|--------|
| `cargo fmt --all --check` | Clean |
| `cargo clippy -p loopflow -- -D warnings` | Clean |
| `cargo test -p loopflow` | 466 passed, 1 ignored, 0 failed |
| No `unwrap()` outside tests | Confirmed |
| No dead code / debug prints | Confirmed |
| No TODOs / FIXMEs in new code | Confirmed |
| Sync bridge fully removed | Confirmed (no `RunStore`, `run_store`, `as_run_store`) |
