# Async Store Boundary (Stage 02)

## Summary

Stage 02 is complete. `lfd` now uses one async storage path end-to-end at call sites:

- `SharedStore` is `Arc<Store>`.
- The sync bridge (`RunStore`, `run_store`, `Store::as_run_store`, `Store::into_shared`) is removed.
- Runtime code in HTTP routes, executor, and triggers awaits store methods directly.

## Problem this solved

Before this stage, each DB operation crossed sync/async boundaries multiple times:

1. HTTP routes wrapped store access with `run_store(...)` and DB-focused `spawn_blocking`.
2. `PostgresStore` owned a tokio runtime and used `block_on`.
3. Async capability traits delegated back into sync `RunStore` calls.

That added latency and made ownership of blocking work hard to reason about.

## Final architecture

- `Store` remains the shared enum wrapper and async boundary used by runtime code.
- Capability traits (`WaveStateStore`, `ExecutionStore`, `StoreAdmin`) remain the async contract.
- Backend behavior is explicit:
  - **PostgresStore** uses direct async `tokio-postgres` calls (no embedded runtime bridge).
  - **SqliteStore** keeps rusqlite internally but isolates sync work with `tokio::task::spawn_blocking` inside backend methods.

This keeps callers uniform (`await` everywhere) while containing unavoidable sqlite blocking inside the backend.

## Scope delivered

- Removed sync bridge types/helpers.
- Migrated shared-handle usage to `Arc<Store>`.
- Migrated HTTP + executor + triggers/scheduler call sites to async store calls.
- Updated store/http/executor tests to async-first patterns.

## Decisions and boundaries

- One async path only; no compatibility shim.
- SQLite replacement is out of scope for this stage.
- No behavior changes to wave semantics/scheduling policy.
- Stage 03+ work (shared SQL catalog, prompt pipeline changes, executor decomposition) remains out of scope.

## Validation performed

- `cargo fmt --all --check`
- `cargo clippy -p loopflow -- -D warnings`
- `cargo test -p loopflow lfd::store`
- `cargo test -p loopflow lfd::http`
- `cargo test -p loopflow lfd::executor`
- `cargo test -p loopflow`
- Grep checks confirm removal of sync bridge and postgres runtime bridge.

## Current risks

- SQLite still serializes through a mutex-protected connection; heavy write bursts may queue on the blocking pool.
- Broad migration surface increases regression risk in less-traveled paths.
- SQLite blocking-task join failures currently map to `InvalidData`; if seen in production, error classification may need refinement.
