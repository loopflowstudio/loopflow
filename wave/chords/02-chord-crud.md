# 02: Chord CRUD

Domain type, store operations, HTTP API, and Python client for chord management.

## Status

Shipped on this branch.

## What shipped

### Rust domain + store

- Added `lfd::types::Chord` with `id`, `name`, `is_default`, `created_at`
- Added dedicated `ChordStore` capability on `Store` with:
  - `create/get/list/delete_chord`
  - `add/remove_chord_member`
  - `list_chord_members`, `list_chords_for_wave`
- Implemented parity in both SQLite and Postgres backends
- Membership add/remove operations are idempotent

### HTTP API

| Method | Route | Behavior |
|--------|-------|----------|
| POST | `/v0/chords` | Create chord; duplicate name returns `409` |
| GET | `/v0/chords` | List chords |
| GET | `/v0/chords/:id` | Get single chord |
| DELETE | `/v0/chords/:id` | Delete chord, `204` |
| POST | `/v0/chords/:id/members` | Add member, `204`, idempotent |
| DELETE | `/v0/chords/:id/members/:wave_id` | Remove member, `204`, idempotent |
| GET | `/v0/chords/:id/members` | List chord members (returns `WaveDto[]`) |
| GET | `/v0/waves/:wave_id/chords` | List chords a wave belongs to |

Unknown chord/wave operations return `404`. Unified `parse_lfd_id()` helper handles ID validation across all chord and session routes.

### Python client/API

- Added `Chord` model in `python/loopflow/models.py`
- Added client methods for chord CRUD + membership mutation + membership reads
- Added `list_chord_members()` and `list_wave_chords()` to client and top-level API
- Added top-level wrappers in `python/loopflow/api.py`

### Docs

- README and `docs/lfd.md` updated with membership listing examples

### Validation

- `cargo fmt --all`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test --all`
- `uv run pytest python/tests/`
- `swift test --package-path swift`
- `tests/e2e/test_smoke.sh`
- `uv run pytest tests/e2e/test_api_smoke.py -v`

## Decisions locked in

- Keep chord operations in a dedicated `ChordStore` trait (not `WaveStateStore`)
- Keep chords orthogonal to execution/stimulus semantics
- Defer default chord auto-enrollment

## Follow-ups

- Add dedicated Postgres integration tests for chord membership paths
- Revisit membership mutation pre-check reads if profiling shows DB pressure
- Keep duplicate-name conflict mapping aligned with backend constraint naming
- Consider lightweight member payload option (wave ID + name only) for large chords where full WaveDto enrichment is expensive
- Add pagination/filtering for chord member and wave-chord list endpoints
