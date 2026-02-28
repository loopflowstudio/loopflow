# Review: Daemon Data Integrity

Branch: `jack-heart.infra.20260228_1357`

## What was implemented

Three independent data integrity improvements for the lfd daemon:

1. **Per-migration transactions** — Each SQLite migration runs inside `BEGIN EXCLUSIVE` / `COMMIT` with rollback on failure. Postgres migrations changed from one caller-provided transaction wrapping all migrations to per-migration transactions via `&mut Client`. A comment documents the duplicate `016_` prefix. Integration test verifies all 23 migrations apply cleanly, key tables exist, and re-application is idempotent.

2. **Resource accumulation bounds** — Three unbounded collections now have cleanup:
   - `prune_output_logs()` deletes `*.log` files older than a configurable TTL (default 7 days). Runs at startup and hourly via a cancellation-aware background task.
   - `OutputHub::close_writer()` drops file handles when wave runs reach terminal state (both success and failure paths in `WaveExecutor`).
   - `remove_reconcile_lock()` removes `QUEUE_RECONCILE_LOCKS` entries when a wave is deleted.

3. **Webhook startup warning** — `warn!` when `webhook_secret` is empty. The endpoint already returns 503, so this is operator visibility, not a behavior change.

## Key choices

| Decision | Why |
|----------|-----|
| `BEGIN EXCLUSIVE` over `BEGIN IMMEDIATE` | Prevents other connections from reading half-migrated schema. Migrations run once at startup — the lock is brief. |
| `&mut Client` instead of `&Transaction` for Postgres | Enables per-migration transactions. `tokio_postgres::Transaction` auto-rolls back on drop, so error paths are clean. |
| Closure-based rollback in SQLite | Ensures ROLLBACK fires if INSERT or COMMIT fails, not just if migration SQL fails. The original `match` only caught `execute_batch(migration.sql)` errors; `?` in the Ok branch would skip rollback. |
| File handle cleanup on terminal state, not LRU | Simpler, predictable. Active runs keep handles; completed runs don't. |
| mtime-based pruning, not log rotation | Design spec called for TTL pruning. mtime is reliable and doesn't require tracking state. |

## How it fits together

The three changes are independent — each touches different modules with no shared state. The migration change affects `store/migrations.rs` and `store/postgres.rs`. Resource bounds affect `output.rs`, `executor/wave/mod.rs`, `queue.rs`, and `http/routes/waves.rs`. The webhook warning is a single check in `bin/lfd.rs`. Config plumbing for `output_log_retention_days` threads through `config.rs` → `RawLfdConfig` → `LfdConfig` → `bin/lfd.rs`.

## Risks and bottlenecks

- **`prune_output_logs` is synchronous** — it does blocking filesystem I/O on the async runtime. For typical output directories (hundreds of files), this is negligible. If directories grow to tens of thousands of files, it could block the runtime briefly. The startup call is before the server starts, so it's safe there. The hourly call is on a spawned task but still blocking — could be wrapped in `spawn_blocking` if it ever becomes a problem.
- **No env override for `output_log_retention_days`** — other config fields like `webhook_secret` have `LFD_*` env overrides. This field only supports YAML config. Fine for an initial implementation since the 7-day default is reasonable.

## What's not included

- Log rotation/compression (out of scope per design)
- Webhook rate limiting (out of scope)
- Migration versioning overhaul or 016_ renumbering (intentionally deferred)
- Env override for `output_log_retention_days`

## Gate fix applied

Fixed incomplete rollback in `apply_sqlite`: the original code only rolled back if `execute_batch(migration.sql)` failed. If the subsequent `INSERT INTO schema_migrations` or `COMMIT` failed, the `?` operator would return without rolling back, leaving an open transaction. Wrapped the entire transaction body in a closure so any error triggers rollback.
