# Design Review: Postgres Storage Backend + LfdId + UI Polish

## What was implemented

**LfdId unified ID type:**
- `LfdId` newtype in `rust/lfd/src/id.rs` with UUID generation and validation
- Serialization traits: `Display`, `FromStr`, `Serialize`, `Deserialize`
- SQLite support via `rusqlite::ToSql` / `FromSql`
- Postgres support via `tokio_postgres::ToSql` / `FromSql`
- Store trait methods updated to use `&LfdId` instead of `&str`
- Proto boundary converts `String` <-> `LfdId` at service layer

**Rust Postgres backend:**
- `PostgresStore` implementing the full `RunStore` trait (42 methods)
- Schema migration system (`lfd migrate` / `lfd migrate --status`)
- Docker Compose setup for local development with Postgres
- Config-driven backend selection via `LFD_STORAGE` environment variable
- Dual-backend test suite (SQLite + Postgres with testcontainers)

**Concerto UI updates:**
- Wave detail panel redesigned with action bar (Land, IDE, Terminal, Stop, Abandon)
- WaveSidebar renamed "Agents" with categorized sections (Blocked, Open PRs, Active, Idle)
- New DesignSystem.swift additions for button styles

## Key choices

**LfdId with `from_raw` escape hatch:** `LfdId::parse()` validates UUIDs, but step-run and fork-run IDs use composite strings (e.g., `"{wave_id}:{step_index}"`). The `LfdId::from_raw()` method bypasses validation for these internal IDs. This keeps type safety at the API boundary while accommodating existing ID formats. See `scratch/questions.md` for discussion of migrating these to UUIDs.

**Sync trait over async trait:** The `RunStore` trait uses synchronous methods. `PostgresStore` creates its own tokio runtime and uses `block_on` to bridge async postgres calls. This preserves trait compatibility with the existing `SqliteStore` implementation and avoids forcing async across the entire codebase.

**TEXT primary keys, not UUID:** The Postgres schema uses `TEXT PRIMARY KEY` rather than native `UUID`. This matches SQLite behavior where IDs come from the Rust code as strings. Simpler migration path, consistent with existing code.

**Connection pool via deadpool-postgres:** Chose a connection pool (max 16 connections) over per-query connections. The pool handles concurrent requests without connection churn.

**Transactions for counter updates:** `create_pending_activation` and `delete_pending_activations` use transactions to atomically update the `pending_activations` counter on the waves table. This prevents counter drift.

**Schema versioning via meta table:** A simple key-value `meta` table tracks schema version. `lfd migrate` is idempotent—runs all migrations up to current, no-ops if already current.

## How it fits together

```
main.rs
├── LFD_STORAGE=postgres → PostgresStore::connect_async(LFD_DATABASE_URL)
├── LFD_STORAGE=sqlite (default) → SqliteStore::new(db_path)
└── Both implement RunStore trait
    └── Used by ControlServer, Scheduler, loops
```

The Postgres backend is fully parallel to SQLite. No code outside the store layer knows which backend is in use.

Docker setup:
```
docker-compose.yml
├── postgres (health check waits for ready)
└── lfd (depends on postgres, runs migrate on start)
```

## Risks and bottlenecks

**Runtime in PostgresStore:** Each `PostgresStore` instance owns a tokio runtime. If the daemon already runs in a tokio context, this creates nested runtimes. The code handles this via `tokio::runtime::Handle::try_current()` but it's architecturally awkward. Future work could make the trait async.

**No retry on connection failure:** Pool errors bubble up immediately. Network hiccups could cause transient failures. Consider adding retry logic for production deployments.

**Schema divergence:** The actual migration SQL differs slightly from the schema documented in `scratch/rust-data-backend.md` (docs show UUID columns, actual uses TEXT). This is intentional for SQLite parity but docs should be updated.

**Testcontainers in CI:** The postgres test suite uses testcontainers, which requires Docker. CI will need Docker enabled for these tests.

## What's not included

- Multi-tenancy (Phase 2: `tenant_id`, `project_id` columns)
- Events table (append-only audit log)
- Retention policies and cleanup jobs
- SQLite-to-Postgres data migration tooling
- TLS/SSL support for Postgres connections
- Per-entity ID types (`WaveId`, `StepRunId`, etc.)

## Testing

- SQLite test suite: passes
- Postgres test suite: passes (via testcontainers or external Postgres)
- `LFD_SKIP_POSTGRES_TESTS=1` skips Postgres tests when Docker unavailable
- Rust format/clippy: clean

## Files changed

| File | Change |
|------|--------|
| `rust/lfd/src/id.rs` | New: LfdId newtype with UUID generation, validation, serialization |
| `rust/lfd/src/store/postgres.rs` | New: 979 lines implementing PostgresStore |
| `rust/lfd/src/store/mod.rs` | Added ForkRun, ForkRunStatus, StoreError variants, LfdId in trait methods, test suite |
| `rust/lfd/src/store/sqlite.rs` | Updated to use LfdId in queries |
| `rust/lfd/src/store/migrations/postgres/001_initial.sql` | New: initial schema |
| `rust/lfd/Cargo.toml` | Added tokio-postgres, deadpool-postgres, testcontainers deps |
| `rust/lfd/src/main.rs` | Backend selection logic, `migrate` subcommand |
| `rust/lfd/src/server.rs` | StoreError::Postgres handling, LfdId::parse at proto boundary |
| `rust/lfd/src/loops/*.rs` | Updated to use LfdId |
| `rust/lfd/Dockerfile` | New: multi-stage build for lfd binary |
| `rust/lfd/docker-compose.yml` | New: local dev with Postgres |
| `rust/lfd/README.md` | New: docker usage docs |
| `scratch/lfd-id.md` | Design doc for LfdId |
| `scratch/questions.md` | Open question about composite IDs |
| `roadmap/rust/05-data-backend.md` | Deleted: replaced by scratch/ design |
