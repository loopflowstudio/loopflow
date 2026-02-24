# Chords Data Model — Current State

## Scope

Chords are now first-class waves: a wave can be a solo voice or a chord with persistent child waves.

This document is the canonical scratch note for the branch. It replaces earlier split notes and review fragments.

## Landed decisions and implementation

### Wave type model

- `Wave` is now an enum:
  - `Wave::Voice(WaveData)`
  - `Wave::Chord { data, children }`
- Shared fields live in `WaveData`.
- Accessor methods (`id()`, `name()`, `repo()`, `data()`, `data_mut()`, `children()`, etc.) replaced direct struct field usage at call sites.

### Persistence and invariants

Migration `011_chords_data_model.sql` rebuilds wave/stimulus storage with chord fields and constraints:

- `wave_type`, `parent_wave_id`, `position`
- Parent/child tree as self-referential FK (`ON DELETE CASCADE`)
- Name uniqueness by scope:
  - top-level: `(repo, name)` where `parent_wave_id IS NULL`
  - child: `(parent_wave_id, name)` where `parent_wave_id IS NOT NULL`
- Stimulus and parenthood are mutually exclusive (nested waves cannot own stimuli)

### Store loading

- Subtrees load via one recursive CTE, then assemble in memory.
- Depth is capped (`MAX_CHORD_DEPTH = 8`) and overflow is rejected.

### Run entrypoint guard

- Nested waves are explicitly rejected for direct run (`409`) rather than rerouted.

## API redesign: join/leave

### Motivation

The original `create_chord` API was top-down: declare the full chord with all voices upfront. This is cumbersome — voices are independent agents with their own flows, and grouping should be bottoms-up.

### `join(wave_a, wave_b)` — the only creation primitive

One verb. Two waves in, one chord out. Behavior depends on inputs:

| Input A | Input B | Result |
|---------|---------|--------|
| Voice | Voice | New chord created, both reparented into it |
| Chord | Voice | Voice absorbed into existing chord |
| Voice | Chord | Voice absorbed into existing chord |
| Chord | Chord | All children of B merged into A, B deleted |

**Absorb, not nest.** `join` always produces a flat chord. Nesting (sub-chords) would be a separate explicit operation if ever needed.

**Store operation:** `UPDATE waves SET parent_id = ?, position = ? WHERE id = ?`. No tree recreation — just reparent.

### `leave(wave)` — pull a voice out

Removes a voice from its chord. The voice becomes solo and inert (no stimulus).

- If the chord drops to one remaining child, it becomes a single-voice chord (not auto-dissolved).
- The leaving voice has no stimulus by default — add one explicitly to make it run independently.

### Stimulus invariants (unchanged)

- Chord owns the stimulus, voices never do.
- Joining strips any existing stimulus from the voice.
- Leaving produces a solo voice with no stimulus.
- A solo voice needs an explicit stimulus to become runnable.

### Store simplification

`create_chord` disappears from the store trait. `create_wave` walks children:

```rust
pub fn create_wave(&self, wave: &Wave) -> StoreResult<()> {
    self.upsert_wave(wave)?;
    for child in wave.children() {
        self.create_wave(child)?;
    }
    Ok(())
}
```

`join` and `leave` are reparenting operations, not creation operations.

### Python API

```python
join("designer", "infra")           # creates chord, both absorbed
join("ensemble", "vocalist")        # vocalist joins existing chord
leave("vocalist")                   # vocalist becomes solo, no stimulus
```

### HTTP routes

```
POST /v0/waves/join    { "wave_a": "designer", "wave_b": "infra" }
POST /v0/waves/leave   { "wave": "vocalist" }
```

## Current invariants (source of truth)

- Voice waves cannot have children.
- Child wave names are unique within a parent chord.
- Nested waves cannot own triggers/stimuli.
- Runs begin from top-level trigger-owning waves/chords.
- Unknown/invalid wave kind data fails loudly.
- Joining strips stimulus from the absorbed voice.
- Leaving produces a solo voice with no stimulus.
- A single-voice chord is valid (not auto-dissolved).

## Changes remaining in this diff

### 1. Add `nest` parameter to `join`

`join(a, b, nest=false)` — when `nest=true`, B becomes a child of A instead of its children being merged flat.

| nest | Input A | Input B | Result |
|------|---------|---------|--------|
| false | Voice | Voice | New chord, both reparented (current) |
| false | Chord | Voice | Voice absorbed (current) |
| false | Chord | Chord | B's children merged into A, B deleted (current) |
| true | Voice | Voice | New chord, both reparented (same as false) |
| true | Chord | Voice | Voice absorbed (same as false) |
| true | Chord | Chord | B reparented as child of A (nesting) |

`nest` only changes behavior for chord+chord. Voice+voice and chord+voice are always absorb — nesting a single voice is meaningless.

**Store:** Add `nest: bool` parameter to `join_waves`. When true and both are chords, reparent B directly into A as a child (don't flatten). B keeps its children.

**HTTP:** `POST /v0/waves/join { "wave_a": "...", "wave_b": "...", "nest": true }`

**Python:** `join("ensemble-a", "ensemble-b", nest=True)`

### 2. BeatGrid: metadata on Chord (phase 04 design decision)

Beat grid is an `Option<Vec<Vec<bool>>>` on the Chord variant — a direct matrix:

```rust
Chord { data: WaveData, children: Vec<Wave>, beats: Option<Vec<Vec<bool>>> }
// beats[beat_index][child_index] = active
```

No grid = all-on every tick (backwards compatible). With a grid, the chord plays through beats sequentially.

BeatGrid invariants:
- No silent beats — every beat must have at least one active child
- Single-child beat grids are valid (repeat semantics)

**What this means for this diff:** The `nest` parameter on `join` is the preparation — it makes nesting possible, which beat grids need. The `beats` field itself is phase 04 work.

## Remaining work (later phases)

1. **Execution semantics (phase 02):** best-effort descendant execution.
2. **Listen step (phase 03):** inter-voice communication.
3. **Beat grid (phase 04):** BeatGrid variant, sequential execution, drum machine UI.

## Out of scope for this branch

- Voicing template schema/type work
- Chord management CLI UX expansions
- Backward compatibility for pre-010 storage layout
- BeatGrid implementation (phase 04)
