# Chords Phase 01 — Design Review

## What was implemented

Wave is now an enum (`Voice(WaveData)` | `Chord { data, children }`) with accessor methods replacing direct field access across the entire codebase. Two new operations — `join` and `leave` — provide bottoms-up chord composition. Migration 011 rebuilds wave/stimulus storage with chord-aware constraints.

## Key changes

**Type model.** `Wave` is a tagged enum with shared `WaveData`. Accessors (`id()`, `name()`, `repo()`, `data()`, `data_mut()`, `children()`) give both variants a uniform interface. `#[non_exhaustive]` reserves room for future variants (Rhythm).

**Store layer.** Recursive CTE loads subtrees in one query, `assemble_wave_tree()` builds nested `Wave` values in memory. Row mapping is shared between SQLite and Postgres via the `StoreRow` trait. `join_waves()` and `leave_wave()` are reparenting operations — no tree recreation. Depth capped at `MAX_CHORD_DEPTH = 8`.

**HTTP routes.** `POST /v0/waves/join` and `POST /v0/waves/leave` with validation: can't join nested waves, can't join during active runs, can't join cross-repo waves, can't leave a wave that isn't in a chord. Appropriate status codes (400, 404, 409, 412).

**Python API.** `loopflow.join("a", "b")` and `loopflow.leave("v")` with `Wave` model extended for `wave_type`, `parent_id`, `position`, `children`.

**Migration 011.** Full table rebuild with `wave_type`, `parent_wave_id` (self-referential FK, `ON DELETE CASCADE`), `position`. Partial unique indices enforce name uniqueness by scope (top-level: `(repo, name)`; child: `(parent_wave_id, name)`).

**Accessor migration.** Every call site that touched `wave.field` now uses `wave.field()` — executor, triggers, queue, HTTP routes, summaries, sidecars, forks.

## How it fits together

```
Wave enum (types/wave.rs)
  ├─ WaveData — shared fields
  ├─ Voice(WaveData) — solo wave
  └─ Chord { data, children } — composite wave

Store (store/mod.rs, sqlite.rs, postgres.rs)
  ├─ rows.rs — StoreRow trait + shared map_wave_row / assemble_wave_tree
  ├─ WaveStateStore trait — join_waves, leave_wave, load_wave_tree
  └─ reparented_wave / new_chord_wave — shared helpers

HTTP (http/routes/waves.rs)
  ├─ join_waves_handler — validates, delegates to store, returns WaveDto
  └─ leave_wave_handler — validates, delegates to store, returns WaveDto

Python (api.py, client.py, models.py)
  ├─ join(wave_a, wave_b) → Wave
  └─ leave(wave) → Wave
```

## Key choices

**Bottoms-up join/leave over top-down create_chord.** Voices are independent agents with their own flows — grouping should reflect that. `join` absorbs, `leave` extracts. No top-down declaration of the full chord.

**Flat absorption on join, not nesting.** `join(chord, voice)` absorbs the voice into the existing chord. `join(chord_a, chord_b)` merges B's children into A. Nesting would be a separate explicit operation.

**Single-voice chords are valid.** When a voice leaves, the chord doesn't auto-dissolve. This avoids surprising state changes and lets chords be "parking lots" for future voices.

**Stimulus stripping on join.** Joining strips any stimulus from the absorbed voice — the chord owns the stimulus. Leaving produces a solo voice with no stimulus. Both are explicit design choices to prevent orphaned triggers.

**Table rebuild in migration 011.** No existing customer data to protect. Clean schema with proper constraints is worth more than incremental ALTER TABLEs.

## Risks and bottlenecks

**Recursive CTE performance.** Tree loading uses a single recursive CTE per wave fetch. For shallow trees (depth <= 8) with few children, this is fine. Could become a bottleneck if chord sizes grow large — but the depth cap provides a natural bound.

**Accessor method ergonomics.** The `wave.field()` pattern is more verbose than `wave.field`. This is the cost of enum-based modeling. The accessor migration was the largest mechanical change in the branch.

**Flaky test.** `wave_rename_renames_branch` in `wave_worktree_tests.rs` is intermittently failing due to what appears to be a race condition in `worktree_move` + `branch_rename`. Pre-existing — not introduced by this branch.

## What's not included

- **Execution semantics (phase 02):** inherited trigger tick, parallel child execution, failure recording.
- **Listen step (phase 03):** inter-voice communication via PR digestion.
- **Voicing templates:** schema-to-wave instantiation is deferred.
- **CLI UX for chords:** `lfq` doesn't surface chord structure yet.
