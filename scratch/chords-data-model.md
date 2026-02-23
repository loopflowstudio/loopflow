# 01: Data Model

Extend the Wave type to support chords — a wave that contains other waves — with recursive SQLite storage and the voicing concept for instantiation.

## What exists after this

`Wave` is an enum with `Voice` and `Chord` variants. Chords persist in SQLite with their child waves. The voicing concept (direction, area, model, parameters) is explicit in the data model, connecting what fork drafts already do implicitly to the chord abstraction.

## What to build

### Wave enum

```rust
enum Wave {
    Voice(WaveData),
    Chord { data: WaveData, waves: Vec<Wave> },
}
```

- A Chord can be used anywhere a Wave can be used
- Recursive: a chord can contain chords
- A nested chord is opaque to its parent
- Derive the standard traits; ensure serialization handles recursion

### SQLite storage

- Store the recursive Wave enum — likely a `waves` table with a nullable `parent_wave_id` for the tree structure
- Each wave row carries its `WaveData` plus a `wave_type` discriminator (voice/chord)
- Load/save round-trips correctly for nested chords
- Depth limit to prevent unbounded recursion in storage queries

### Voicing

- Make voicing explicit in the data model: the choices made when instantiating a wave schema (direction, area, model, parameters)
- Connect to existing fork draft semantics — fork drafts with different directions are voicings
- An instantiated wave has at most one parent chord (or none if solo)

## What we'll learn

- Whether a single `waves` table with self-referential `parent_wave_id` is sufficient or if we need a separate `chord_members` join table
- How deep nesting realistically goes and whether we need recursion limits
- Whether voicing needs its own table or is just fields on the wave row

## Open questions

- Schema migration path from current wave storage
- Whether `WaveData` needs new fields for chord-specific metadata
- Index strategy for parent lookups in deeply nested chords

## Done when

- `Wave` enum compiles with Voice and Chord variants
- SQLite round-trip: create a chord with nested voices, persist, reload, verify structure
- Nested chord (chord containing a chord) persists and loads correctly
- Voicing fields are present and populated for instantiated waves
- Existing solo wave behavior is unchanged (Voice variant = current behavior)
