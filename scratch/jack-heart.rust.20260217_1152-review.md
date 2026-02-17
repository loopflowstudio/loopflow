# Async Store Boundary (Stage 02) — Review Guide

## What was implemented

- Removed the sync storage bridge (`RunStore`, `run_store`, `Store::as_run_store`, `Store::into_shared`) and moved all call sites to async `Store` methods.
- Switched shared storage handle to `SharedStore = Arc<Store>`.
- Updated `PostgresStore` to native async DB access (no embedded runtime, no `block_on`).
- Kept `SqliteStore` on rusqlite, but isolated blocking through `spawn_blocking` at the backend boundary.
- Migrated HTTP routes, executor paths, and trigger/scheduler flows to await store calls directly.
- Updated store/http/executor tests to use the async-first interface.

## Key choices

- **One async path only**: removed compatibility shims in the same PR to avoid dual-interface drift.
- **Backend-specific blocking strategy**:
  - Postgres: direct async queries.
  - SQLite: blocking work wrapped internally so callers still use a uniform async API.
- **Keep capability traits (`WaveStateStore`, `ExecutionStore`, `StoreAdmin`)** as the contract, with `Store` delegating by backend.

Alternatives rejected:
- Keeping `RunStore` temporarily (would preserve boundary complexity).
- Introducing a second transitional async trait (would duplicate APIs).
- Replacing rusqlite now (out of scope for Stage 02).

## How it fits together

`open_store` now returns a single `Store` enum wrapper (sqlite or postgres). Runtime code receives `Arc<Store>` and always calls async methods. Internally, `Store` delegates to backend implementations: postgres executes async queries directly; sqlite runs sync operations in blocking tasks. This keeps the call graph async end-to-end at integration boundaries while containing unavoidable blocking inside sqlite backend internals.

## Risks and bottlenecks

- **SQLite contention under load**: sqlite still serializes through a mutex-protected connection; heavy concurrent write bursts can queue on the blocking pool.
- **Large migration surface**: broad call-site changes across HTTP/executor/triggers increase regression risk for less-traveled paths.
- **Error mapping from blocking tasks**: sqlite task join failures surface as `InvalidData`; if these appear in production, additional classification may be useful.

Validation run for this branch:
- `cargo fmt --all --check`
- `cargo clippy -p loopflow -- -D warnings`
- `cargo test -p loopflow lfd::store`
- `cargo test -p loopflow lfd::http`
- `cargo test -p loopflow lfd::executor`
- `cargo test -p loopflow`
- grep checks confirming no `RunStore`/`run_store` bridge and no postgres runtime bridge

## What's not included

- Shared SQL catalog unification (Stage 03).
- Prompt pipeline/document-source work (Stage 04).
- Executor decomposition or scheduling policy changes.
- SQLite backend replacement with a fully async driver.
