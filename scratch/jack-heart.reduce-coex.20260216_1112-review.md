# Gate Review: Store trait scope reset (Stage 1)

## What was implemented
- Added `StorageConfig`, `Store`, and `StoreBackend` to centralize backend selection/opening.
- Added grouped capability traits in `lfd::store`:
  - `WaveStateStore`
  - `ExecutionStore`
  - `StoreAdmin`
- Implemented those traits on `Store` by delegating to the existing `RunStore` backend implementations.
- Added `open_store(...)` and `migrate_store(...)` helpers so startup and migrate command paths share backend logic.
- Updated `lfd` binary wiring to use the new store helpers.
- Kept existing `RunStore` trait as the transition shim so existing executor/http code remains unchanged.
- Updated store tests to exercise `open_store(...)` in the sqlite suite.
- Added `docs/lfd.md` migration command docs and backend-selection behavior.
- Tightened startup config UX: invalid `LFD_STORAGE` now returns a clear error instead of silently falling back to sqlite.

## Key choices
- **Keep `RunStore` in place for Stage 1** to avoid broad call-site churn while introducing capability boundaries.
- **Put backend setup/migration in top-level helpers** (`open_store`, `migrate_store`) so binary code no longer reaches directly into sqlite/postgres constructors for common flows.
- **Use one `StorageConfig` enum for both runtime and migration flows** so backend selection logic is defined once.
- **Validate `LFD_STORAGE` values explicitly** (`sqlite`/`postgres`) to prevent accidental misconfiguration.

## How it fits together
`lfd` now parses env into `StorageConfig`, then calls `migrate_store` (when needed) and `open_store`. `Store` wraps the concrete backend and exposes grouped async capability traits that delegate to the existing synchronous `RunStore` implementation. Existing runtime components still use `SharedStore = Arc<dyn RunStore>` during this stage.

## Risks and bottlenecks
- `RunStore` still exists and remains the active interface in executor/http paths; async/sync boundary cleanup is still pending future stages.
- Capability traits are currently wired through delegation to the sync trait, so this stage improves API shape but not runtime blocking behavior yet.
- `Store::into_shared()` drops the wrapper and returns the legacy trait object; migration of call sites to capability traits is deferred.

## What's not included
- No executor/http refactor to consume `WaveStateStore`/`ExecutionStore` directly.
- No removal of `run_store`/`spawn_blocking` paths.
- No SQL catalog unification between sqlite/postgres.
- No workspace domain model (`WaveWorkspace`) work.
- No executor module split.
