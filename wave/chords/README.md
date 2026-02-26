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
