# Branch Review: jack-heart.rust.20260217_1253

## What was implemented

- Completed the storage-layer simplification stages by shipping a shared SQL catalog and async-first store usage end-to-end.
- Added `lfd/store/catalog.rs` with one query catalog consumed by both sqlite and postgres backends.
- Refactored `sqlite.rs` and `postgres.rs` to fetch SQL from the catalog instead of maintaining duplicated inline query strings.
- Kept runtime call sites (`http`, `executor`, `triggers`) on direct async `Store` methods and aligned tests around the async boundary.

## Key choices

- **Single catalog, explicit dialect rendering**: SQL is authored once with `{pN}` placeholders and rendered to sqlite/postgres placeholder formats.
- **Explicit divergence, not hidden abstraction**: true dialect differences stay as named per-query overrides.
- **Cache rendered SQL**: rendered statements are stored lazily to avoid runtime re-rendering on hot paths.
- **No compatibility shim reintroduced**: the branch keeps one async path and does not revive the old sync bridge patterns.

## How it fits together

`http` routes, triggers, and executor code call async `Store` APIs directly. `Store` dispatches to sqlite/postgres backends, and backend methods now bind values against catalog-provided SQL (dialect-rendered once). This preserves backend-specific driver behavior while centralizing query intent in one module.

## Risks and bottlenecks

- SQLite still depends on blocking DB work (`rusqlite` + mutex + blocking pool), so high write contention can queue.
- Query correctness now depends on catalog entry integrity; parity tests reduce drift risk but every new query must be added to the catalog correctly.
- Dialect overrides are intentionally explicit but can still regress if backend binding assumptions change without matching tests.

## What's not included

- No replacement of sqlite/postgres drivers.
- No executor decomposition work.
- No prompt pipeline (`DocumentSource`) redesign from Stage 04.
- No product-level behavior changes to waves/scheduling semantics.

## Validation run

- `cargo fmt --all -- --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test --all`
- `uv run pytest python/tests/`
- `swift test --package-path swift`
- `cd swift && xcodegen generate && xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS'`
- `tests/e2e/test_smoke.sh`
