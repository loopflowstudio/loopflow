# Chords

Named groups of waves with inter-wave listening. Chords organize related waves and let them react to each other via the `listen` stimulus.

**Model:** Waves are flat. Chords group them. Listening connects them.

## North Star

A user creates waves, groups them into a chord, and wires `listen` stimuli so waves react to each other's work. No nested execution engine, no inherited triggers — just grouping and stimulus-based coordination.

## Design Decisions

**Waves are flat.** No enum, no tree. A `Wave` is a single struct. Chords are a separate concept stored in their own tables (`chords` + `chord_members`). A wave can belong to multiple chords or none.

**Chords are groups, not executors.** A chord doesn't own a trigger or manage child lifecycle. It's a named collection. Execution is still per-wave, driven by each wave's own stimuli.

**Listening is a stimulus.** `StimulusKind::Listen` with a `source_wave_id` fires a wave when its source completes. No special iteration-aware scheduling — it's just another stimulus type alongside loop/cron/watch/once.

**Flat over nested.** The recursive model (chord-as-wave, nested chords, inherited triggers) moved to Symphonia/studio. OSS keeps the simpler model where composition happens through stimulus wiring, not tree structure.

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

## Phases

| # | Phase | Focus | Status |
|---|-------|-------|--------|
| 01 | Flatten + Listen | Drop tree model, add listen stimulus, migration 012 | shipped |
| 02 | Chord CRUD | Domain type, store ops, HTTP API, Python client | |
| 03 | Listen Authoring | Listen stimulus in wave schema files, deeper inter-wave communication | |
| 04 | Concerto UI | Chord groups and listen wiring in the macOS app | |

### Phase 01 retrospective

The original plan called for a recursive `Wave` enum (Voice | Chord) with join/leave composition, nested execution, and inherited triggers. Building it revealed that the recursive model was over-engineered for the OSS use case — users want named groups and inter-wave reactivity, not a tree-structured wave scheduler.

The pivot: flatten `Wave` to a single struct, drop `wave_type`/`parent_id`/`position`/`join`/`leave`, add `chords`/`chord_members` tables for grouping, and add `StimulusKind::Listen` with `source_wave_id` for inter-wave coordination. Migration 012 handles the schema transition. Python client updated to match (removed `join()`/`leave()`, added `source_wave_id` to stimulus API).

Key insight: stimulus-based listening (`source_wave_id`) is simpler and more composable than the nested iteration model. A wave can listen to any other wave regardless of chord membership. Chords provide organization; stimuli provide coordination. These are orthogonal.

## Future Directions

### Nested Chord Orchestration (Symphonia)

The recursive model — inherited triggers, child scheduling, beat-grid execution — lives in Symphonia/studio. If OSS needs it later, the `chords`/`chord_members` schema is a clean foundation to build on, but the execution engine stays simple here.

### Listen Step (post-chord CRUD)

Once chord CRUD is in place, a dedicated `listen` step could inject sibling PR content at iteration start — richer inter-wave communication than just "source completed, fire me." This builds on the stimulus wiring but adds content awareness.

### Multi-user (lfd-hub)

- lfd stays simple: one user, one machine, local chords
- lfd-hub (private codebase) orchestrates across lfd instances
- The listen stimulus protocol is the clean seam — lfd-hub can inject remote activations the same way lfd handles local ones

## Open Questions

- Should new waves auto-enroll in a default chord?
- Should `listen` stimuli be accepted in wave schema files now, or remain API-only until chord CRUD lands?

## Done When (wave complete)

- Chords exist as named groups with membership CRUD
- HTTP API and Python client support chord operations
- Listen stimulus wires waves to react to each other
- Concerto shows chord grouping and listen relationships
