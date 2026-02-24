# Chords: Simple Groups + Listening

Complex chord orchestration (beat grids, inherited triggers, parallel execution) → Symphonia/studio repo.
Open-source loopflow gets simple chord groups + listen stimulus.

## Design

**Chord** = named group of waves that listen to each other.

**Default chord**: All waves on an lfd instance are in one default chord. They all listen to each other automatically. Explicit chords can be created for subgroups.

**ListeningFlow** (replaces "sidecar"): A wave run triggered by listening to another wave. Has:
- `source` — what it listens to (a wave, chord, or repo event)
- `flow` — what it does when the source speaks

**Stimulus source hierarchy** (user's product framing):
- **chord listening** — primary, encouraged for users to tinker with
- **github** — automatic-on (webhook triggers)
- **repo** — power-user flexibility (direct repo watching)

## What's Done (Rust)

Wave flattened from enum (Voice/Chord) to simple struct. All chord tree machinery removed:

- [x] `Wave` is a flat struct (no more Voice/Chord enum, WaveData, parent_id, position)
- [x] Removed `MAX_CHORD_DEPTH`, `reparented_wave()`, `new_chord_wave()`
- [x] Removed `join_waves()`, `leave_wave()` from Store trait + impls (sqlite, postgres)
- [x] Removed `StoreError::DepthLimitExceeded`, `NestedWaveCannotOwnStimulus`, `StimulusOwnerCannotBeNested`
- [x] Removed `WaveTreeRow`, `assemble_wave_tree()`, recursive CTE queries
- [x] Removed `/waves/join` and `/waves/leave` HTTP routes
- [x] Removed `JoinWavesRequest`, `LeaveWaveRequest`, `ensure_wave_can_run_directly()`
- [x] Removed `WaveChildDto`, `wave_type`, `parent_id`, `position`, `children` from DTOs
- [x] Updated SQL catalog: wave queries use 12 columns (no wave_type, parent_wave_id, position)
- [x] All `wave.data_mut().field` → `wave.field` across executor, triggers, HTTP handlers
- [x] All tests pass (545 tests), clippy clean, fmt clean

## What's Left

### SQL migration (Step 2)
`012_remove_chord_tree.sql`:
- Drop `wave_type`, `parent_wave_id`, `position` columns from waves
- Drop chord-related indexes
- Add `chords` table: `id TEXT PK, name TEXT NOT NULL UNIQUE, is_default INTEGER NOT NULL DEFAULT 0, created_at BIGINT NOT NULL`
- Add `chord_members` table: `chord_id TEXT FK → chords(id), wave_id TEXT FK → waves(id), PK (chord_id, wave_id)`
- Add `source_wave_id TEXT` column to `stimuli` table (for listen kind)
- Add `Listen = 5` stimulus kind

### Chord type + store ops (Step 6)
New `chord.rs` type:
```rust
pub struct Chord {
    pub id: LfdId,
    pub name: String,
    pub is_default: bool,
    pub created_at: Option<OffsetDateTime>,
}
```

Store methods: `create_chord`, `delete_chord`, `add_chord_member`, `remove_chord_member`, `list_chords`, `get_chord`, `get_default_chord`, `list_chords_for_wave`.

Default chord behavior: auto-add new waves to the default chord on creation.

### Listen stimulus (Step 5)
- Add `Listen = 5` to `StimulusKind`
- Add `source_wave_id: Option<LfdId>` to `Stimulus` (populated for Listen kind)
- Rename sidecar concept → ListeningFlow
- ListeningFlow has a `source` and a `flow`

### Chord HTTP routes (Step 4b)
- `POST /chords` — create chord
- `GET /chords` — list chords
- `GET /chords/:id` — get chord with members
- `DELETE /chords/:id` — delete chord
- `POST /chords/:id/members` — add wave
- `DELETE /chords/:id/members/:wave_id` — remove wave

### Rename sidecars → ListeningFlows (Step 5b)
- `SidecarKind` → concept absorbed into Listen stimulus
- `WaveRunKind::Sidecar` → `WaveRunKind::Listening`
- `sidecar_kind` field → remove or repurpose
- Update all references

### Python API (Step 7)
- Remove `join()`, `leave()` from API and client
- Remove `wave_type`, `parent_id`, `position`, `children` from Wave model
- Add `Chord` model and chord API methods

### Documentation (Step 9)
- Remove `wave/chords/` directory (moves to studio)
- Remove `scratch/chords-execution.md`
- This file (`scratch/chords-simple.md`) captures the new model
