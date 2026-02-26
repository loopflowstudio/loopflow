# Chord CRUD: Flat Grouping API

## Problem

`chords` and `chord_members` exist in the database but are not usable from domain code, HTTP, or Python.

Without CRUD, the Chords wave goal **"Chords exist as named groups with membership CRUD"** is blocked, and later goals (**"HTTP API and Python client support chord operations"**, Concerto chord UI, listen authoring UX) cannot ship.

Who benefits now:
- CLI/API users who need to organize many waves
- Python automation users scripting wave ensembles
- Future Concerto UI work that needs stable chord endpoints

Why now:
- Phase 01 shipped schema + listen stimulus foundation
- Phase 03/04 depend on a stable chord data surface

## Approach

Build first-class chord support end-to-end with a dedicated storage facet and explicit HTTP routes.

1. **Rust domain type**
   - Add `lfd/types/chord.rs` with:
     - `Chord { id: LfdId, name: String, is_default: bool, created_at: Option<OffsetDateTime> }`
   - Derive `Debug`, `Clone`, `Serialize`, `Deserialize`
   - Export from `lfd/types/mod.rs`

2. **Store layer**
   - Add a new `ChordStore` trait on `Store` (parallel to `WaveStateStore`, `ExecutionStore`, `SessionStore`), with:
     - `create_chord`, `get_chord`, `list_chords`, `delete_chord`
     - `add_chord_member`, `remove_chord_member`
     - `list_chord_members`, `list_chords_for_wave`
   - Add `Store` convenience methods forwarding to `ChordStore`.
   - Implement in both `sqlite.rs` and `postgres.rs` using explicit SQL.
   - Membership mutations are **idempotent**:
     - add existing member => no-op success
     - remove missing member => no-op success

3. **HTTP API**
   - Add `http/routes/chords.rs` and wire into router:
     - `POST /v0/chords`
     - `GET /v0/chords`
     - `GET /v0/chords/{id}`
     - `DELETE /v0/chords/{id}`
     - `POST /v0/chords/{id}/members`
     - `DELETE /v0/chords/{id}/members/{wave_id}`
   - Add DTOs for chord payloads.
   - Return `204 No Content` for delete/add-member/remove-member endpoints.
   - Error contract:
     - `404` for unknown chord or wave
     - `409` for duplicate chord name on create

4. **Python client + API wrappers**
   - Add `Chord` model in `python/loopflow/models.py`
   - Add `Client` methods: `create_chord`, `list_chords`, `get_chord`, `delete_chord`, `add_chord_member`, `remove_chord_member`
   - Add matching top-level functions in `python/loopflow/api.py`
   - Handle `204` responses cleanly in client request handling.

5. **Tests**
   - Rust store tests: create/list/get/delete chord + membership add/remove/list for SQLite suite
   - Rust HTTP route tests: endpoint happy paths + duplicate/not-found errors
   - Python tests: model parsing + client request/response behavior for chord methods

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Extend `WaveStateStore` with chord methods | Fewer traits, fewer files touched | Makes an already-large trait broader and less coherent; chords are a separate concern |
| Add chord CRUD only at HTTP layer with inline SQL in handlers | Fastest short-term | Duplicates DB logic, bypasses store abstraction, weak backend parity guarantees |
| Auto-enroll every new wave into default chord now | Faster "it works out of the box" story | Violates wave risk guidance (default enrollment ambiguity), removes explicit opt-in grouping |

## Key decisions

- **Use a dedicated `ChordStore` trait** to keep `WaveStateStore` focused and make backend parity explicit.
- **Treat chords as organization only**, not execution semantics, to stay aligned with wave goal **"Chords are groups, not executors"** and avoid Symphonia overlap.
- **Defer default chord auto-enrollment** (keep `is_default` persisted/readable, but no automatic join logic).
- **Do not cascade into stimuli behavior** when deleting chords; only membership rows are removed (via FK cascade).
- **Idempotent membership endpoints** reduce retry pain in automation and avoid spurious failures.

Wild success details we are designing for:
- Users script chord setup in one pass (safe to re-run).
- Concerto can render groups immediately from stable list/get APIs.
- Listen relationships remain orthogonal to chord membership.

Wild failure we are actively preventing:
- Chords accidentally become a hidden execution tree.
- "Default chord" surprises users by making every wave implicitly grouped.
- Membership endpoints become flaky due to duplicate insert races.

## Scope

- In scope:
  - Rust `Chord` type
  - Store CRUD + membership operations for SQLite and Postgres
  - Six HTTP chord endpoints
  - Python model/client/api wrappers for chord CRUD + membership mutations
  - Focused Rust + Python tests for chord behavior

- Out of scope:
  - Auto-enrolling new waves into a default chord
  - Any listen stimulus semantics change
  - Concerto chord UI work (Phase 04)
  - Nested or orchestrated chord execution semantics

## Done when

- Wave goals advanced:
  - **"Chords exist as named groups with membership CRUD"**
  - **"HTTP API and Python client support chord operations"**
- Commands pass:
  - `cargo test -p loopflow store::tests::sqlite_store_basic_suite`
  - `cargo test -p loopflow chord`
  - `uv run pytest python/tests/test_models.py python/tests/test_client.py -k chord`
- Observable API behavior:
  - `POST /v0/chords` creates a chord
  - `POST /v0/chords/{id}/members` and `DELETE /v0/chords/{id}/members/{wave_id}` both return `204`
  - `GET /v0/chords` and `GET /v0/chords/{id}` return chord data with `is_default` and `created_at`
