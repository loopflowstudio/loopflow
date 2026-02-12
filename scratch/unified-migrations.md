# Unified Migrations

One migration system for both sqlite and postgres. No existing users, clean start.

## What to build

A shared migration runner that applies numbered `.sql` files to both backends, with per-backend override support for when SQL dialects diverge.

## Current state

- Postgres: 4 migration files in `migrations/postgres/`, tracked in `schema_migrations` table
- SQLite: monolithic `ensure_schema()` + `ensure_column()` calls, no migration tracking
- Schemas are logically identical but use different SQL types (JSONB vs TEXT, BOOLEAN vs INTEGER)
- ~80% of store code is duplicated between the two backends

## Design

### Migration file layout

```
store/migrations/
  001_initial.sql          # default — used by both unless overridden
  002_something.sql
  003_something/
    default.sql            # used by both unless overridden
    postgres.sql           # postgres-specific override
    sqlite.sql             # sqlite-specific override
```

Resolution: look for `NNN_name/{backend}.sql` first, then `NNN_name/default.sql`, then `NNN_name.sql`. A migration is either a single file (works for both) or a directory with variants.

### Schema conventions

Standardize on portable SQL to minimize overrides:

- `TEXT` for everything (no JSONB) — JSON arrays stored as text, parsed in Rust
- `INTEGER` for bools (0/1) and timestamps (unix seconds)
- `BIGINT` only in postgres overrides if needed (SQLite INTEGER handles any size)
- Foreign keys use `REFERENCES table(col) ON DELETE CASCADE` inline

The current postgres schema uses JSONB for `direction`, `area`, `flow_parents`, `snapshot_direction`, `snapshot_area`, `snapshot_pr`. None of these are queried with JSON operators — they're always fully deserialized. TEXT is sufficient.

### Migration runner

```rust
// In store/migrations.rs (new file)

struct Migration {
    version: String,    // "001_initial"
    sql: String,        // resolved SQL for this backend
}

enum Backend { Sqlite, Postgres }

fn resolve_migrations(backend: Backend) -> Vec<Migration>;
```

Each backend calls `resolve_migrations()` at startup (sqlite) or via `lfd migrate` (postgres). The runner:

1. Creates `schema_migrations` table if not exists
2. Reads applied versions from `schema_migrations`
3. Applies unapplied migrations in order
4. Records each applied migration with timestamp

SQLite runs migrations automatically on `SqliteStore::new()` (same as today's `ensure_schema()` but using the migration system). Postgres continues to require explicit `lfd migrate`.

### Embedding migrations

Use `include_str!()` at compile time. A build script or macro walks `migrations/` and produces the migration list. Simpler option: just list them manually like today's postgres code does, but with the resolution logic.

### What to delete

- `ensure_schema()` in sqlite.rs (replaced by migration runner)
- `ensure_column()` helper (no longer needed — migrations handle schema changes)
- `migrate_stimuli_from_waves()` (no existing data to migrate)
- All 4 existing postgres migration files (replaced by single `001_initial.sql`)
- The `PostgresMigrator` struct and its methods (replaced by shared runner)
- `collapse_migrations.py` script (no longer needed)

### What changes in sqlite.rs / postgres.rs

- `SqliteStore::new()` calls shared migration runner instead of `ensure_schema()`
- `PostgresStore::migrate()` calls shared migration runner
- Remove `BASELINE_MIGRATIONS` / `INCREMENTAL_MIGRATIONS` arrays from postgres.rs
- `schema_version()` reads from `schema_migrations` table (latest applied version)

## Data structures

```rust
// store/migrations.rs

pub struct Migration {
    pub version: String,
    pub sql: String,
}

pub fn resolve_migrations(backend: Backend) -> Vec<Migration> {
    // For each migration, check backend-specific first, then default
    ...
}

pub fn applied_versions_sqlite(conn: &Connection) -> StoreResult<HashSet<String>> { ... }
pub fn apply_migration_sqlite(conn: &Connection, migration: &Migration) -> StoreResult<()> { ... }

// Postgres equivalents that take a tokio_postgres::Transaction
pub async fn applied_versions_postgres(tx: &Transaction<'_>) -> StoreResult<HashSet<String>> { ... }
pub async fn apply_migration_postgres(tx: &Transaction<'_>, migration: &Migration) -> StoreResult<()> { ... }
```

## Constraints

- Migration resolution must happen at compile time (embedded SQL, not runtime file reads)
- SQLite migrations run automatically on connect; postgres requires explicit `lfd migrate`
- A migration that works as `default.sql` should not also have backend-specific overrides — pick one
- Migration order is determined by filename prefix (001, 002, ...)

## Done when

```bash
# Existing tests pass with the new migration system
cargo test -p loopflow store

# Single 001_initial.sql exists and works for both backends
ls rust/loopflow/src/lfd/store/migrations/001_initial.sql

# No ensure_schema() or ensure_column() in sqlite.rs
grep -c "ensure_schema\|ensure_column" rust/loopflow/src/lfd/store/sqlite.rs
# → 0

# No JSONB in any migration file
grep -c "JSONB" rust/loopflow/src/lfd/store/migrations/*.sql
# → 0
```
