# Chord CRUD Review

## What was implemented

- Added a first-class `Chord` domain type in Rust and exported it from `lfd::types`.
- Added store-layer chord CRUD and membership APIs across SQLite and Postgres:
  - `create/get/list/delete_chord`
  - `add/remove_chord_member`
  - `list_chord_members`, `list_chords_for_wave`
- Added HTTP chord routes:
  - `POST /v0/chords`
  - `GET /v0/chords`
  - `GET /v0/chords/{id}`
  - `DELETE /v0/chords/{id}`
  - `POST /v0/chords/{id}/members`
  - `DELETE /v0/chords/{id}/members/{wave_id}`
- Added Python chord support:
  - `Chord` model
  - `Client` methods for chord CRUD + membership mutations
  - top-level `loopflow.api` wrappers
- Added/extended tests:
  - Rust store suite now exercises chord CRUD + idempotent membership behavior
  - Rust chord route tests cover happy paths, duplicate name conflict, missing resources, empty names, invalid IDs
  - Python model/client tests cover chord parsing and chord client behavior

## Key choices

- Kept chords in a dedicated `ChordStore` trait instead of extending `WaveStateStore`, to keep store responsibilities coherent.
- Membership add/remove is idempotent by design to make retries safe in automation.
- Unknown chord/wave membership operations return `404`; duplicate chord names on create return `409`.
- Kept chord grouping orthogonal to execution/stimulus behavior (no implicit execution semantics).
- Tightened Python list-response parsing to raise clear errors on malformed payloads instead of silently returning empty lists.

## How it fits together

`Store` now exposes chord operations through `ChordStore`, with backend-specific SQL in `sqlite.rs` and `postgres.rs`. HTTP handlers in `http/routes/chords.rs` call those store methods and map DB/domain errors to API status codes. Python client wrappers map these endpoints to typed models so CLI/API automation can use the new chord surface directly.

## Risks and bottlenecks

- Duplicate-name conflict detection depends on backend-specific DB error details (constraint name/message matching); schema constraint renames must keep this mapping updated.
- Membership mutations currently pre-check chord/wave existence before writes; this adds extra DB reads for mutation paths.
- Postgres chord operations currently rely on unit coverage through shared route/store tests; no dedicated Postgres integration test was added in this pass.

## What's not included

- No automatic enrollment of new waves into any default chord.
- No chord execution semantics (chords remain grouping metadata only).
- No Concerto UI changes for chord management.
- No new HTTP endpoints for listing members/chords-per-wave (store APIs exist, HTTP surface intentionally scoped to the agreed six routes).

## Wave alignment

This branch advances the wave goals:

- **Chords exist as named groups with membership CRUD**
- **HTTP API and Python client support chord operations**

Validation run in this gate pass:

- `cargo fmt`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test -p loopflow`
- `cargo test -p loopflow store::tests::sqlite_store_basic_suite`
- `cargo test -p loopflow chord`
- `uv run pytest python/tests/`
