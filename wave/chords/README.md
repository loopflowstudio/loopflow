# Chords

## Vision

Named groups of waves with inter-wave listening. Waves are flat. Chords group them. Listening connects them. Not nested orchestration, not multi-user coordination, not approval routing.

## Strategy

The original plan called for a recursive `Wave` enum with nested execution and inherited triggers. Building Phase 01 revealed this was over-engineered — users want named groups and inter-wave reactivity, not a tree-structured scheduler.

The pivot: flatten `Wave` to a single struct, use join tables for grouping, and add `Signal::Listen` with `source_wave_id` for coordination.

**Invariants:**

- Waves are flat — a `Wave` is a single struct, no enum/tree/parent. A wave can belong to multiple chords or none.
- Chords are groups, not executors — no trigger ownership or child lifecycle management.
- Listening is a stimulus — `Signal::Listen` fires when the source completes. Just another signal alongside loop/cron/watch/once/ci_failure.
- Stimulus-based composition over tree structure — a wave can listen to any other wave regardless of chord membership.

### Signal model

Phases 01–03.5 collapsed `WaveRunKind`/`StimulusKind` into a single `Signal` enum. CI fix is a normal stimulus activation (`Signal::CiFailure` + `flow: ci-fix`). `stimulus.flow` override lets any stimulus select a flow at activation time. Non-serialized waves spawn per-run worktrees for parallel execution.

External API still uses `stimulus.kind` — coordinated rename to `stimulus.signal` deferred (requires Python client + Concerto + wave config schema update in lockstep).

### Data model

```rust
struct Wave {
    id: LfdId,
    name: String,
    repo: String,
    // flat fields, no type/parent/position
}

struct Stimulus {
    signal: Signal,  // Once | Loop | Watch | Cron | Listen | CiFailure
    flow: Option<String>,  // override wave.flow for this activation
    source_wave_id: Option<LfdId>,  // for Listen stimuli
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

## Goals

- Chord CRUD works end-to-end: create chord, add/remove waves, list members, delete chord
- Listen stimulus fires reliably when source wave completes
- A wave listening to another wave in a different chord works identically to same-chord listening
- Concerto shows chord grouping and listen relationships
- HTTP API and Python client support chord operations

## Risks

- **Listen fan-out.** Many waves listening to one source triggers N runs simultaneously. No concurrency limiting today. Acceptable at current scale; revisit if fan-out exceeds scheduler capacity.
- **CI recursion guard coupling.** Recursion prevention keys off `snapshot.flow == "ci-fix"`. Renaming the flow without updating the guard reintroduces recursion.
- **Concurrent CI stimulus creation.** CI failure trigger resolves-then-creates `Signal::CiFailure` stimuli. Serialized today by one event loop; needs uniqueness guard if parallelized.
- **Run worktree accumulation.** Parallel runs create per-run worktrees cleaned up on completion. Daemon crash mid-run leaves orphans until janitor sweep.
- **Stale proto definitions.** `control.proto` still references `StimulusKind`. Update when proto becomes active.

## Metrics

- Chord CRUD works end-to-end from Python client and HTTP API
- Listen stimulus fires reliably when source wave completes (including edge cases: source fails, source is stopped)
- Concerto UI shows chord grouping and listen wiring visually
- A wave listening to another wave in a different chord works identically to same-chord listening
