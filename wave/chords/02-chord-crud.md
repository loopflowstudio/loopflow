# 02: Chord CRUD

Domain type, store operations, HTTP API, and Python client for chord management.

## What exists after this

Users can create named chords, add/remove waves as members, list chords, and delete them. The `chords` and `chord_members` tables (from migration 012) are populated through a full CRUD API.

## What Phase 01 established

Migration 012 created the `chords` and `chord_members` tables with proper foreign keys and indexes. The schema supports a default chord (`is_default` flag with unique partial index). No Rust domain type or store operations exist yet — the tables are empty scaffolding.

## What to build

### Rust domain type

- `Chord` struct: `id`, `name`, `is_default`, `created_at`
- Derive `Debug`, `Clone`, `Serialize`, `Deserialize`
- Place in `lfd/types/` alongside `Wave` and `Stimulus`

### Store operations

On the `WaveStore` trait (or a new `ChordStore` trait — decide based on how the existing trait is structured):

- `create_chord(name) -> Chord`
- `get_chord(id) -> Option<Chord>`
- `list_chords() -> Vec<Chord>`
- `delete_chord(id)`
- `add_chord_member(chord_id, wave_id)`
- `remove_chord_member(chord_id, wave_id)`
- `list_chord_members(chord_id) -> Vec<Wave>`
- `list_chords_for_wave(wave_id) -> Vec<Chord>`

Implement for both SQLite and Postgres backends.

### HTTP API

| Method | Route | Body | Returns |
|--------|-------|------|---------|
| POST | `/v0/chords` | `{ "name": "..." }` | Chord |
| GET | `/v0/chords` | — | `Vec<Chord>` |
| GET | `/v0/chords/:id` | — | Chord |
| DELETE | `/v0/chords/:id` | — | 204 |
| POST | `/v0/chords/:id/members` | `{ "wave_id": "..." }` | 204 |
| DELETE | `/v0/chords/:id/members/:wave_id` | — | 204 |

### Python client

- `Chord` model in `models.py`
- Client methods: `create_chord`, `list_chords`, `get_chord`, `delete_chord`, `add_chord_member`, `remove_chord_member`
- API convenience wrappers in `api.py`

### Default chord behavior

Decide: should newly created waves auto-enroll in a default chord? The schema supports `is_default` but the behavior isn't wired yet. Could be deferred if it adds complexity without clear user value.

## Open questions

- Separate `ChordStore` trait or extend `WaveStore`?
- Default chord auto-enrollment: now or later?
- Should chord deletion cascade to member waves' listen stimuli that reference chord members?

## Done when

- `Chord` type exists in Rust with full CRUD in both store backends
- HTTP routes return proper responses for all six endpoints
- Python client can create chords, manage membership, and list
- Existing wave operations are unaffected
- Tests cover create/list/delete and membership mutations
