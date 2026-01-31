# Design Review: Postgres Storage Backend + UI Polish

## What was implemented

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

## Testing

- SQLite test suite: passes
- Postgres test suite: passes (via testcontainers or external Postgres)
- `LFD_SKIP_POSTGRES_TESTS=1` skips Postgres tests when Docker unavailable
- Python tests: 673 passed
- Swift tests: 70 passed
- Rust format/clippy: clean

## Files changed

| File | Change |
|------|--------|
| `rust/lfd/src/store/postgres.rs` | New: 969 lines implementing PostgresStore |
| `rust/lfd/src/store/mod.rs` | Added ForkRun, ForkRunStatus, StoreError variants, test suite |
| `rust/lfd/src/store/migrations/postgres/001_initial.sql` | New: initial schema |
| `rust/lfd/Cargo.toml` | Added tokio-postgres, deadpool-postgres, testcontainers deps |
| `rust/lfd/src/main.rs` | Backend selection logic, `migrate` subcommand |
| `rust/lfd/src/server.rs` | StoreError::Postgres handling |
| `rust/lfd/Dockerfile` | New: multi-stage build for lfd binary |
| `rust/lfd/docker-compose.yml` | New: local dev with Postgres |
| `rust/lfd/README.md` | New: docker usage docs |
| `swift/Concerto/Views/WaveDetailPanel.swift` | Refactored: action bar, abandon confirmation |
| `swift/Concerto/Views/WaveSidebar.swift` | Renamed to Agents, added sections |
| `swift/Concerto/DesignSystem.swift` | Added DarkButtonStyle, OutlineButtonStyle |
| `scratch/rust-data-backend.md` | Design doc for Postgres backend |
| `scratch/questions.md` | Updated with resolved questions |
| `roadmap/rust/05-data-backend.md` | Deleted: replaced by scratch/ design |
