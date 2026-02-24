# Design Creates Waves + Split-Wave — Review

## What was implemented

Three changes, unified by the goal of making wave YAML on disk the source of truth:

1. **Schema abstraction removed** — Deleted the `wave_schemas` HTTP routes (486 lines), built-in wave YAML files, `schema_ref`/`schema_name` database columns, and all associated code in `builtins.rs`, `build.rs`, `dto.rs`, `store/mod.rs`, `docker.rs`, `queue.rs`, and `wave/mod.rs`. Added migration 012 to drop the columns.

2. **`wave_config` module added** — New `wave_config.rs` reads `wave/<name>/<name>.yaml` at wave creation time via `read_wave_config()`. Clean error handling: returns `None` for missing files, logs and returns `None` for parse errors.

3. **`split-wave` ops step** — New prompt for wave mitosis: parent wave is fully consumed and replaced by N children. Added to `BUILTIN_CATEGORIES` in discovery, README ops table, and the `design` step's wave-plan path now writes YAML and roadmap files directly.

4. **`discover_target` tightened** — Now only falls back from step to flow lookup on `StepNotFound`, not on any error. Other errors (parse failures, permission issues) propagate correctly.

## Key choices

- **YAML on disk, not in database**: `lfd` reads wave config at creation time and stores `flow`, `direction`, `area` in the database as runtime state. The YAML file remains the source of truth. This avoids schema versioning complexity while keeping the daemon functional.

- **Migration uses `DROP COLUMN`**: SQLite 3.35.0+ supports this. `rusqlite` bundles a modern SQLite, so this is safe. Older system SQLite installations would fail, but `lfd` doesn't rely on system SQLite.

- **`split-wave` is non-interactive**: Runs as an ops step. The agent reads the parent wave, finds split boundaries, creates children, and deletes the parent. Review happens after, not during.

- **`name` removed from wave YAML**: Directory name (`wave/<name>/`) is canonical. Eliminates a class of name/directory mismatch bugs.

## How it fits together

Waves are defined by `wave/<name>/` directories containing a README.md (vision, goals, risks) and `<name>.yaml` (flow, area, direction, stimulus). `lf design` creates these directories when the user chooses the wave-plan path. `lfd` reads the YAML once via `read_wave_config()` when creating a wave in its store. `split-wave` decomposes a wave directory into N child directories, preserving all roadmap items.

## Risks and bottlenecks

- **Swift client still references `WaveSchema`**: `LocalWaveService.swift`, `WaveSidebar.swift`, `RepoState.swift`, and `WaveServiceProtocol.swift` call the now-removed `GET /wave/schemas` endpoint. The error handling returns `[]` so the app won't crash, but the schema-related UI features are dead code. Follow-up needed to clean up the Swift side.

- **Migration 012 is irreversible**: `DROP COLUMN` can't be undone. If we need schema columns back, we'd need a new migration to re-add them. This is intentional — the direction is away from storing schema info in the database.

## What's not included

- Swift/Concerto cleanup of `WaveSchema` references (separate branch)
- Making `lfd` fully stateless for wave config (future direction per design doc)
- Changes to `add-to-wave` or `wave-plan` steps
