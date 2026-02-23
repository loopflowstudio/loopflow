# Rust Simplification

Reduce complexity in lfd's storage and prompt layers without touching the executor module split.

## North Star

lfd keeps both SQLite and Postgres. Store operations are async end-to-end. Prompt assembly uses typed document sources. Each stage is independently shippable.

## Stages

| # | Stage | What it unlocks | Pre-work | Status |
|---|-------|----------------|----------|--------|
| 01 | Store trait scope reset | Capability traits, `StorageConfig`, `open_store`/`migrate_store` | None | Shipped |
| 02 | Async store boundary | Routes call store directly; no `spawn_blocking` for DB ops | 01 | Shipped |
| 03 | Shared SQL catalog | One query per operation; dialect rendering for `?`/`$N` | 02 | Shipped |
| 04 | Prompt pipeline | `DocumentSource` enum; `gather_documents(specs)` | None | In progress |

Stages 01–03 (the store simplification arc) are complete. Stage 04 is independent — it touches `engine/prompt.rs`, not the store layer. Each stage deletes what it replaces — no deferred cleanup.

## What's shipped

### Stage 01 — Store trait scope reset

- `StorageConfig` enum with `sqlite(path)` / `postgres(url)` constructors
- `Store` wrapper + `StoreBackend` enum centralizing backend selection
- Grouped capability traits: `WaveStateStore`, `ExecutionStore`, `StoreAdmin`
- `open_store()` / `migrate_store()` helpers for startup and migration

### Stage 02 — Async store boundary

- `SharedStore` is `Arc<Store>` (concrete type, not `Arc<dyn RunStore>`)
- `RunStore` trait, `run_store()` helper, `Store::as_run_store()` all deleted
- `Store` exposes `pub async fn` methods directly; callers `await` everywhere
- SQLite blocking contained inside backend via `run_sqlite` + `spawn_blocking`
- Postgres calls `tokio-postgres` directly; embedded runtime removed
- HTTP routes, executor, triggers, and tests all migrated to async-first

### Stage 03 — Shared SQL catalog

- `lfd/store/catalog.rs` defines one `Query` catalog for both backends
- SQL templates use `{pN}` placeholders and render to `?N` (sqlite) or `$N` (postgres)
- Backend-specific syntax is isolated in explicit per-query overrides
- Rendered SQL is cached via lazy static storage
- Catalog parity tests enforce rendering coverage and contiguous placeholder numbering

## Architecture

```
Store layer (shipped):
  HTTP routes ──await──▶ Arc<Store>
  Store dispatches to SqliteStore (spawn_blocking) or PostgresStore (async)
  SQL comes from one catalog, rendered per dialect once

Prompt layer (next):
  Documents typed by DocumentSource enum, assembled via gather_documents(spec)
  Formatting consolidated into one entry point with mode parameter
```

## Out of scope

- Executor module decomposition (`lfd/executor.rs` split) — wave worktree colocation belongs there
- Product behavior changes (wave semantics, flow semantics, scheduler policy)
- Adding/removing storage backends
