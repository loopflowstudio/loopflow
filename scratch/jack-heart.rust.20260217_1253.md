# Storage Layer Simplification: Current State

## Scope

`lfd` storage now runs through one async call path at runtime, and SQL behavior is centralized in one shared catalog for both SQLite and Postgres.

## Architecture snapshot

- `SharedStore` is `Arc<Store>`.
- Runtime code (`http`, `executor`, `triggers`) awaits `Store` methods directly.
- Sync bridge pieces were removed (`RunStore`, `run_store`, `Store::as_run_store`, `Store::into_shared`).
- `Store` remains the async dispatch boundary across backends.
- Backend behavior is explicit:
  - `PostgresStore` uses async `tokio-postgres` directly.
  - `SqliteStore` keeps `rusqlite` but isolates blocking work with `tokio::task::spawn_blocking`.

## SQL model

- Query intent lives in `lfd/store/catalog.rs`.
- Queries are authored once with `{pN}` placeholders.
- Dialect rendering is deterministic:
  - SQLite: `{pN}` → `?N`
  - Postgres: `{pN}` → `$N`
- True SQL divergence is handled as named per-query overrides.
- Rendered SQL is cached lazily.

## Delivered outcomes

- Removed duplicated inline query bodies across `sqlite.rs` and `postgres.rs`.
- Preserved both SQLite and Postgres as first-class backends.
- Kept behavior unchanged for wave scheduling/execution semantics.
- Updated tests around async-first store usage and catalog parity.

## Validation used

- `cargo fmt --all --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test --all`
- `uv run pytest python/tests/`
- `swift test --package-path swift`
- `cd swift && xcodegen generate && xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS'`
- `tests/e2e/test_smoke.sh`

## Known risks

- SQLite write-heavy workloads can still queue behind the mutex + blocking pool model.
- Catalog correctness depends on keeping each new store operation wired into shared query definitions and parity tests.
- Backend binding changes can regress dialect overrides if coverage is not updated with the change.

## Out of scope for this branch

- Replacing SQLite/Postgres drivers.
- Executor decomposition.
- Prompt pipeline redesign (`DocumentSource`).
