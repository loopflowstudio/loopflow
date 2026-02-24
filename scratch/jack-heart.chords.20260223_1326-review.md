# Chords Data Model — Review Guide

## What was implemented

- Replaced the Rust `Wave` struct model with `Wave::Voice(WaveData)` and `Wave::Chord { data, children }`.
- Added wave accessors (`id()`, `name()`, `repo()`, `data()`, `data_mut()`, `children()`, etc.) and migrated call sites to accessor-based usage.
- Added chord persistence fields/invariants across storage backends:
  - `wave_type`, `parent_wave_id`, `position`
  - partial unique indexes for top-level and child name scopes
  - stimulus/parent mutual-exclusion enforcement
  - recursive tree loading with depth checks.
- Added migration `010_chords_data_model.sql` (destructive reset of affected tables with new constraints).
- Added chord creation APIs:
  - HTTP: `POST /v0/chords`
  - Python: `loopflow.create_chord(...)` and `Client.create_chord(...)`
  - DTO/model support for `wave_type`, `parent_id`, `position`, and nested `children`.
- Added/updated tests for chord round-trip loading, nested chord loading, depth limit rejection, and chord API client payloads.
- Gate polish fix in this pass: nested waves are now explicitly rejected in `run_wave_handler` (`409 nested waves cannot be run directly`), plus focused unit tests.

## Key choices

- **Enum over discriminator field:** chord/voice behavior is type-level, with exhaustive matching where behavior differs.
- **DB as invariant source of truth:** uniqueness and parent/stimulus constraints are enforced in schema/triggers, with domain-level error mapping.
- **One-query subtree loading:** recursive CTE + in-memory assembly avoids N+1 subtree fetches.
- **Top-level run entrypoint:** direct run of child/nested waves is explicitly blocked.

## How it fits together

Chord creation writes one top-level chord row plus child voice rows linked by `parent_wave_id` and ordered by `position`. Read paths load top-level waves and reconstruct nested trees via recursive CTE queries. Execution and API layers now consume `Wave` through accessor methods so most code remains variant-agnostic, while chord-aware behavior is handled where needed (store loading, DTO typing, route validation).

## Risks and bottlenecks

- **Migration is destructive:** existing wave/stimulus rows are reset by migration 010 (intentional for this phase).
- **Route coverage is still thin:** most new chord behavior is validated via store tests; HTTP-level chord tests could be expanded.
- **CI environment sensitivity:** local `xcodebuild test -scheme Concerto` run failed in `ConcertoUITests.ScreenshotPipelineTests testCapture` while Rust/Python/Swift package/e2e smoke passed.

## What's not included

- No listen-step orchestration (phase 03) in runtime behavior.
- No chord management CLI UX beyond API/HTTP creation path.
- No voicing template schema/type; voicing remains represented by existing `flow`/`direction`/`area` fields.
- No backward-compat path for pre-010 storage layout.
