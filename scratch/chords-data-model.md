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

Migration `010_chords_data_model.sql` rebuilds wave/stimulus storage with chord fields and constraints:

- `wave_type`, `parent_wave_id`, `position`
- Parent/child tree as self-referential FK (`ON DELETE CASCADE`)
- Name uniqueness by scope:
  - top-level: `(repo, name)` where `parent_wave_id IS NULL`
  - child: `(parent_wave_id, name)` where `parent_wave_id IS NOT NULL`
- Stimulus and parenthood are mutually exclusive (nested waves cannot own stimuli)

### Store loading

- Subtrees load via one recursive CTE, then assemble in memory.
- Depth is capped (`MAX_CHORD_DEPTH = 8`) and overflow is rejected.

### API and route surface

- Python API gained `create_chord(...)`.
- Client/model/DTO support includes chord metadata (`wave_type`, `parent_id`, `position`) and nested children.
- HTTP route support includes chord creation (`POST /v0/chords`).

### Run entrypoint guard

- Nested waves are explicitly rejected for direct run (`409`) rather than rerouted.

## Current invariants (source of truth)

- Voice waves cannot have children.
- Child wave names are unique within a parent chord.
- Nested waves cannot own triggers/stimuli.
- Runs begin from top-level trigger-owning waves/chords.
- Unknown/invalid wave kind data fails loudly.

## Remaining work

1. **Execution semantics completion (phase 02):** ensure full best-effort descendant execution behavior is consistently enforced and documented.
2. **Listen-step behavior (phase 03):** descendant awareness/orchestration is still pending.
3. **HTTP-level coverage:** add more route tests for chord behavior (most deep validation is currently store-level).
4. **Concerto UI CI flake:** `ConcertoUITests.ScreenshotPipelineTests testCapture` remained environment-sensitive in prior validation.

## Out of scope for this branch

- Voicing template schema/type work
- Chord management CLI UX expansions
- Backward compatibility for pre-010 storage layout
