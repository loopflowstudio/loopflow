# Branch Review: jack-heart.chords.20260225

## What was implemented

- Added chord membership read APIs in `lfd`:
  - `GET /v0/chords/{id}/members`
  - `GET /v0/waves/{wave_id}/chords`
- Exposed those APIs in the Python client and top-level API:
  - `Client.list_chord_members()` / `loopflow.list_chord_members()`
  - `Client.list_wave_chords()` / `loopflow.list_wave_chords()`
- Added route-level and client-level tests for the new read paths, plus invalid-ID/not-found coverage.
- Unified LfdId parsing in HTTP routes via shared `parse_lfd_id()` and reused it in session parsing.
- Polished docs examples in `README.md` and `docs/lfd.md` to include membership/chord listing flows.

## Key choices

- Reused existing store methods (`list_chord_members`, `list_chords_for_wave`) instead of adding new query layers.
- Returned full `WaveDto` for chord-member listing (via `build_wave_dtos(..., include_active_run=false)`) to match existing wave payload shape while avoiding active-run expansion overhead.
- Kept membership read endpoints on existing resource hierarchy (`/chords/{id}/members`, `/waves/{id}/chords`) to keep API discoverable and symmetric.
- Centralized `LfdId` parse error mapping to reduce route-by-route duplication and keep bad-ID behavior consistent.

## How it fits together

Store backends (SQLite/Postgres) already support chord↔wave membership listing. The HTTP router now wires GET routes to those store calls and returns list DTOs. Python client methods call those routes and parse into `Wave`/`Chord` models, and the top-level `loopflow.api` forwards to the client so scripts can use the new reads without dropping to raw HTTP.

## Risks and bottlenecks

- `list_chord_members` builds full wave DTOs, which still performs per-wave enrichments (git state, flow steps, stimuli); large chords can be heavier than a minimal member payload.
- Duplicate-name conflict mapping still depends on backend-specific constraint metadata (existing known risk in chord CRUD).
- Membership mutation/read paths still perform existence checks before mutation/read; correct behavior, but extra DB round trips remain on hot paths.

## What's not included

- No changes to chord execution semantics (chords remain grouping metadata only).
- No UI changes in Concerto for rendering these new list APIs.
- No pagination/filtering additions for chord member lists.
- No schema changes or migration changes in this polish pass.

## Validation run

- `cargo fmt --all`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test --all`
- `uv run pytest python/tests/`
- `swift test --package-path swift`
- `tests/e2e/test_smoke.sh`
- `uv run pytest tests/e2e/test_api_smoke.py -v`
- `cd swift && xcodegen generate && xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO`
