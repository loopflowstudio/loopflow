# Rust Simplification

Reduce complexity in lfd's storage and prompt layers without touching the executor module split.

## North Star

lfd keeps both SQLite and Postgres. Store operations are async end-to-end. Prompt assembly uses typed document sources. Each stage is independently shippable.

## Stages

| # | Stage | What it unlocks | Pre-work | Status |
|---|-------|----------------|----------|--------|
| 01 | Store trait scope reset | Capability traits, `StorageConfig`, `open_store`/`migrate_store` | None | Shipped |
| 02 | Async store boundary | Routes call store directly; no `spawn_blocking` for DB ops | 01 | |
| 03 | Shared SQL catalog | One query per operation; dialect rendering for `?`/`$N` | 02 | |
| 04 | Prompt pipeline | `DocumentSource` enum; `gather_documents(specs)` | None | |

Stage 04 is independent of the store work and can run in parallel with 02/03. Each stage deletes what it replaces — no deferred cleanup.

## What's shipped (Stage 01)

- `StorageConfig` enum with `sqlite(path)` / `postgres(url)` constructors
- `Store` wrapper + `StoreBackend` enum centralizing backend selection
- Grouped capability traits: `WaveStateStore`, `ExecutionStore`, `StoreAdmin`
- `open_store()` / `migrate_store()` helpers for startup and migration
- Config UX: strict mode profiles select storage/executor/backend together
- `RunStore` kept so executor/http compile unchanged (Stage 02 deletes it)

## Architecture (current → target)

```
Current (Stage 01 shipped):
  HTTP routes ──spawn_blocking──▶ run_store(Arc<dyn RunStore>)
  PostgresStore holds embedded tokio::Runtime for block_on
  Store wrapper exposes async traits, delegates to sync RunStore

Target (Stage 04 complete):
  HTTP routes ──await──▶ Arc<dyn WaveStateStore + ExecutionStore + StoreAdmin>
  Both backends implement async traits natively
  SQL defined once per query, rendered per dialect
  Documents typed by source, assembled declaratively
```

## Out of scope

- Executor module decomposition (`lfd/executor.rs` split) — wave worktree colocation belongs there
- Product behavior changes (wave semantics, flow semantics, scheduler policy)
- Adding/removing storage backends
