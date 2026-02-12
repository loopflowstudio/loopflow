# Unified Store Migrations — Design Review

## What was implemented

Replaced two divergent schema management systems (SQLite's monolithic `ensure_schema()` + `ensure_column()` and Postgres's 4-file `PostgresMigrator`) with a single shared migration runner. Both backends now use the same numbered `.sql` files, embedded at compile time via `include_str!()`.

Extracted duplicated row-mapping logic from both backends into a shared `rows.rs` module using a `StoreRow` trait that abstracts `rusqlite::Row` and `tokio_postgres::Row`.

**Files changed (net -565 lines):**

| File | Change |
|------|--------|
| `migrations.rs` | New — shared migration runner for both backends |
| `rows.rs` | New — `StoreRow` trait + shared row mappers |
| `001_initial.sql` | Unified schema (portable SQL, no JSONB) |
| `postgres.rs` | -771 lines of row mapping, migration code |
| `sqlite.rs` | -914 lines of `ensure_schema()`, row mapping |
| `schema.rs` | Deleted — replaced by `schema_migrations` table |
| `postgres/002-004_*.sql` | Deleted — folded into single `001_initial.sql` |
| `collapse_migrations.py` | Deleted — no longer needed |

## Key choices

**Portable SQL over backend-specific SQL.** All JSONB columns replaced with TEXT. None were queried with JSON operators — they're always fully deserialized in Rust. This eliminates the need for per-backend migration overrides today while preserving the directory-based override mechanism for the future.

**`StoreRow` trait over code generation.** A simple trait with `text()`, `int()`, `bigint()` methods (plus `opt_` variants) abstracts the type differences between SQLite (all integers are i64) and Postgres (i32/i64 distinction). This let row mappers (`map_wave_row`, `map_agent_row`, etc.) be written once and shared.

**`include_str!()` over build scripts.** Migrations are listed manually in `ALL_MIGRATIONS`. Simpler than a proc macro or build script, and the migration list is the single source of truth.

**`schema_migrations` table over `meta` table.** Standard migration tracking pattern. Version + applied_at timestamp per migration. Both backends check table existence before querying.

## How it fits together

```
SqliteStore::new()  ──→  migrations::apply_sqlite()  ──→  001_initial.sql
PostgresStore::migrate_async()  ──→  migrations::apply_postgres()  ──→  001_initial.sql

Both backends:  query → rows::map_*_row() → domain types
```

`migrations.rs` owns the migration list and runner logic (separate sync/async paths for SQLite/Postgres). `rows.rs` owns row-to-struct conversion. `sqlite.rs` and `postgres.rs` own SQL query construction and connection management only.

## Risks and bottlenecks

**`schema_migrations.applied_at` uses INTEGER.** In postgres, this is 32-bit signed — overflows in 2038. Fine for an internal tracking column but inconsistent with the BIGINT convention used for user-facing timestamps. Low risk, easy to fix in a future migration.

**No transaction wrapping for SQLite migrations.** `apply_sqlite` runs each migration with `execute_batch` without an explicit transaction. SQLite's `execute_batch` auto-commits each statement. If a multi-statement migration partially fails, the database could be left in an inconsistent state. Low risk with the current single-migration setup, but worth adding explicit transaction wrapping when more migrations are added.

## What's not included

**Backend enum / resolution logic.** The design doc described a `Backend` enum and file-based resolution (`NNN_name/{backend}.sql`). Since all current SQL is portable, this was deferred — the `migrations()` function returns the flat list directly, and the doc comment explains where override resolution would go.

**Existing data migration.** No migration path from old schemas. The design doc explicitly states "no existing users, clean start."
