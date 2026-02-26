# Chords

Named groups of waves with inter-wave listening. Chords organize related waves and let them react to each other via the `listen` stimulus.

## Vision

A user creates waves, groups them into a chord, and wires `listen` stimuli so waves react to each other's work. No nested execution engine, no inherited triggers — just grouping and stimulus-based coordination.

Waves are flat. Chords group them. Listening connects them.

### Not here

- Nested chord orchestration (inherited triggers, child scheduling, beat-grid execution) — lives in Symphonia/studio
- Multi-user chord coordination — lives in lfd-hub
- Approval routing for cross-wave side effects

## Goals

- Waves are flat: a `Wave` is a single struct, no enum/tree/parent. A wave can belong to multiple chords or none.
- Chords are groups, not executors: no trigger ownership or child lifecycle management. Execution is per-wave via each wave's own stimuli.
- Listening is a stimulus: `StimulusKind::Listen` with `source_wave_id` fires when the source completes. Just another stimulus type alongside loop/cron/watch/once.
- Stimulus-based composition over tree structure: a wave can listen to any other wave regardless of chord membership. Chords provide organization; stimuli provide coordination. These are orthogonal.
- HTTP API and Python client support chord operations
- Concerto shows chord grouping and listen relationships

## Risks

- **Schema migration if chord model evolves.** The `chords`/`chord_members` tables are simple join tables now. If chords gain execution semantics later, the schema may need non-trivial migration. Mitigate: keep the Symphonia execution model separate; don't add execution fields to the chord tables.
- **Listen stimulus fan-out.** If many waves listen to the same source, a single completion triggers N wave runs simultaneously. No concurrency limiting exists today. Mitigate: acceptable at current scale (single user, handful of waves); revisit if fan-out exceeds scheduler slot capacity.
- **Listen trigger scan cost.** Completion-triggered listener lookup currently scans listen stimuli and filters in memory. Fine at current scale; add source-indexed lookup if cardinality grows.
- **Deferred-activation latency.** Pending activations drain on a fixed interval, so deferred listener starts are eventually consistent, not instant.
- **Creation-order coupling for listeners.** Listen source resolution is eager and FK-backed; source waves must exist before listener waves are created.
- **Duplicate-name conflict mapping is backend-detail sensitive.** Phase 02 maps duplicate chord names to `409`, but detection currently depends on backend-specific DB error details. If schema/constraint names change, this mapping can drift.
- **Membership mutation paths do pre-check reads.** Membership add/remove validates chord and wave existence before mutation; this is correct but adds DB round-trips on hot paths.

## Metrics

- Chord CRUD works end-to-end: create chord, add/remove waves, list members, delete chord
- Listen stimulus fires reliably when source wave completes (including edge cases: source fails, source is stopped)
- Concerto UI shows chord grouping and listen wiring visually
- A wave listening to another wave in a different chord works identically to same-chord listening

## Phases

| # | Phase | Focus | Status |
|---|-------|-------|--------|
| 01 | Flatten + Listen | Drop tree model, add listen stimulus, migration 012 | shipped |
| 02 | Chord CRUD | Domain type, store ops, HTTP API, Python client | shipped |
| 03 | Listen Authoring | Listen stimulus in wave schema files, listener triggering, CI fix rename | shipped |
| 04 | Concerto UI | Chord groups and listen wiring in the macOS app | next |

### Phase 01 retrospective

The original plan called for a recursive `Wave` enum (Voice | Chord) with join/leave composition, nested execution, and inherited triggers. Building it revealed that the recursive model was over-engineered for the OSS use case — users want named groups and inter-wave reactivity, not a tree-structured wave scheduler.

The pivot: flatten `Wave` to a single struct, drop `wave_type`/`parent_id`/`position`/`join`/`leave`, add `chords`/`chord_members` tables for grouping, and add `StimulusKind::Listen` with `source_wave_id` for inter-wave coordination. Migration 012 handles the schema transition. Python client updated to match (removed `join()`/`leave()`, added `source_wave_id` to stimulus API).

### Phase 02 retrospective

Phase 02 shipped end-to-end chord CRUD, membership management, and membership read APIs:

- Rust `Chord` type plus `Store` support via dedicated `ChordStore` capability
- SQLite + Postgres implementations for chord CRUD and membership operations
- HTTP API routes for chord CRUD, add/remove member mutations, and membership listing (`GET /v0/chords/:id/members`, `GET /v0/waves/:id/chords`)
- Unified `parse_lfd_id()` helper for consistent ID validation across routes
- Python `Chord` model and client/API wrappers for chord operations including `list_chord_members()` and `list_wave_chords()`
- Route/store/client tests covering happy paths, not-found/conflict behavior, and membership read paths

Key decisions now locked:

- Chords stay grouping metadata only (no execution semantics)
- Membership mutations are idempotent for safe retries
- Default chord auto-enrollment is deferred; grouping remains explicit/opt-in
- Membership reads return full `WaveDto` (via `build_wave_dtos` with `include_active_run=false`)

Carry-forward follow-ups:

- Add dedicated Postgres integration coverage for chord membership paths
- Consider replacing membership pre-check reads with single-statement, constraint-driven mutations if profiling shows pressure
- Consider lightweight member payload for large chords (full WaveDto enrichment can be heavy)
- Add pagination/filtering for membership list endpoints

### Phase 03 retrospective

Phase 03 shipped schema authoring for `stimulus.kind: listen` and made listen execution real:

- Added schema fields `source` + optional `source_repo`
- Added repo-scoped listen source resolution during wave creation (`source` name/ID → `source_wave_id`)
- Added executor-side listener triggering on successful source completion
- Added pending activation queue/coalescing + drain loop for deferred starts
- Renamed sidecar terminology to CI fix terminology, including DB migration 016
- Scoped CI webhook target updates to main runs only (exclude CI-fix runs)

Key decisions now locked:

- Eager source resolution/validation (fail fast before wave persistence)
- Success-only triggering (`Completed` only)
- Queue deferred activations instead of dropping them
- Keep context injection out of Phase 03 (designed, not implemented)

Carry-forward follow-ups:

- Add source-run context injection for listeners (`none | summary | full`)
- Consider failure-triggered listening mode
- Add cycle detection for chained listen graphs
- Consider multi-source listen schema
- Add source-indexed listener lookup if in-memory filtering becomes hot

## Data Model

```rust
struct Wave {
    id: LfdId,
    name: String,
    repo: String,
    // ... flat fields, no type/parent/position
}

struct Stimulus {
    kind: StimulusKind,  // Once | Loop | Watch | Cron | Listen
    source_wave_id: Option<LfdId>,  // for Listen stimuli
    // ...
}
```

```sql
CREATE TABLE chords (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    is_default INTEGER NOT NULL DEFAULT 0,
    created_at BIGINT NOT NULL
);

CREATE TABLE chord_members (
    chord_id TEXT NOT NULL REFERENCES chords(id) ON DELETE CASCADE,
    wave_id TEXT NOT NULL REFERENCES waves(id) ON DELETE CASCADE,
    PRIMARY KEY (chord_id, wave_id)
);
```

## Future Directions

### Listen context (post-Phase 03)

Phase 03 delivered listener triggering and queue reliability. The next extension is context-rich triggering: inject source-run summary (PR title, changed files, optional diff depth) into listener runs so "listen" carries signal, not just timing.

### Multi-user (lfd-hub)

- lfd stays simple: one user, one machine, local chords
- lfd-hub (private codebase) orchestrates across lfd instances
- The listen stimulus protocol is the clean seam — lfd-hub can inject remote activations the same way lfd handles local ones
