# Chords in OSS Loopflow: Current State + Next Work

Complex nested chord orchestration (inherited triggers, child scheduling, beat-grid execution) moved out of OSS scope to Symphonia/studio.

OSS loopflow keeps a simpler model:
- chords are named groups of waves
- waves can listen to other waves via stimulus
- no tree-structured wave scheduler in lfd

## Product decision

- **Primary interaction:** chord/listen behavior users can tinker with directly.
- **Supporting sources:** GitHub and repo events remain available.
- **No nested execution runtime in OSS:** child-chord scheduling and inherited-trigger iteration are not part of this branch's target model.

## Implemented on this branch

- Flattened `Wave` from enum/tree model to a single struct.
- Removed legacy chord-tree concepts from Rust types, store, SQL mapping, and HTTP DTOs:
  - `wave_type`, `parent_id`, `position`, `children`
  - `/v0/waves/join` and `/v0/waves/leave`
  - tree-specific store errors and assembly logic
- Added `listen` stimulus support in Rust:
  - `StimulusKind::Listen = 5`
  - optional `source_wave_id` on `Stimulus`
  - `source_wave_id` persisted in sqlite/postgres and exposed in API DTOs
- Added migration `012_remove_chord_tree.sql`:
  - drops wave-tree columns/indexes
  - adds `chords` + `chord_members`
  - adds `stimuli.source_wave_id`
- Updated Python client/docs to match backend:
  - removed `join()` / `leave()` wrappers for removed routes
  - added `source_wave_id` support for stimuli and examples

## Remaining work

1. **Chord type + store operations**
   - Add `Chord` domain type.
   - Implement `create/delete chord`, membership mutation, chord listing/get operations.
   - Auto-enroll newly created waves into the default chord.

2. **Chord HTTP API**
   - `POST /chords`
   - `GET /chords`
   - `GET /chords/:id`
   - `DELETE /chords/:id`
   - `POST /chords/:id/members`
   - `DELETE /chords/:id/members/:wave_id`

3. **Listen authoring parity**
   - Decide whether schema files should support `listen` + `source_wave_id` now or remain API-only until a later step.

4. **Terminology cleanup**
   - Finish sidecar → listening naming cleanup wherever stale names remain.

5. **Python chord API**
   - Add `Chord` model and chord CRUD/membership client methods.

## Known risks / migration notes

- Existing callers using Python `join/leave` must migrate.
- Concerto UI test `ScreenshotPipelineTests/testCapture` is flaky/failing in this environment; Swift package tests pass.
- Migration 012 is required for databases before running code that assumes flattened wave schema.

## Out of scope for OSS

- Inherited-trigger nested chord scheduler.
- Parent/child chord runtime lifecycle management.
- Chord beat-grid iteration orchestration.
