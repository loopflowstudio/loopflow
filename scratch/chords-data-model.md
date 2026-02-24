# Chords Data Model

## Problem

Waves are solo. You can fork a wave's flow (run the same step with 3 different directions), but fork branches are ephemeral — they live in temp worktrees, execute, synthesize, and disappear. There's no persistence across iterations, no awareness between voices, no hierarchy.

Chords make waves compositional. A chord is a wave that contains other waves. The children persist, iterate, and (in phase 03) listen to each other. This is the foundation for multi-agent collaboration at the wave level.

## Approach

Replace the `Wave` struct with a `Wave` enum (Voice | Chord). Shared fields live in `WaveData`. Accessor methods on `Wave` keep callsites clean — `wave.name` becomes `wave.name()`, a mechanical migration across ~27 files.

### Wave enum

```rust
/// Shared fields for all wave types.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WaveData {
    pub id: LfdId,
    pub name: String,
    pub repo: String,
    pub flow: String,
    pub direction: Vec<String>,
    pub area: Vec<String>,
    pub status: WaveStatus,
    pub iteration: u32,
    pub schema_ref: Option<String>,
    pub schema_name: Option<String>,
    #[serde(with = "time::serde::rfc3339::option")]
    pub created_at: Option<OffsetDateTime>,
    pub parent_id: Option<LfdId>,
    pub position: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "wave_type")]
#[non_exhaustive]
pub enum Wave {
    Voice(WaveData),
    Chord {
        #[serde(flatten)]
        data: WaveData,
        children: Vec<Wave>,
    },
}
```

Accessor methods eliminate the match-everywhere problem:

```rust
impl Wave {
    pub fn data(&self) -> &WaveData {
        match self {
            Wave::Voice(d) | Wave::Chord { data: d, .. } => d,
        }
    }

    pub fn data_mut(&mut self) -> &mut WaveData {
        match self {
            Wave::Voice(d) | Wave::Chord { data: d, .. } => d,
        }
    }

    // Convenience accessors for the most common fields
    pub fn id(&self) -> &LfdId { &self.data().id }
    pub fn name(&self) -> &str { &self.data().name }
    pub fn repo(&self) -> &str { &self.data().repo }
    pub fn status(&self) -> WaveStatus { self.data().status }
    pub fn is_chord(&self) -> bool { matches!(self, Wave::Chord { .. }) }

    pub fn children(&self) -> &[Wave] {
        match self {
            Wave::Voice(_) => &[],
            Wave::Chord { children, .. } => children,
        }
    }
}
```

Why enum, not struct:
- **Exhaustive matching.** Phases 02 (execution) and 03 (listen) need fundamentally different behavior for chords vs voices. The compiler forces you to handle both.
- **Impossible states are unrepresentable.** A Voice can't have children. A struct with `children: Vec<Wave>` allows it.
- **The wave plan already says enum.** Matching the north star avoids drift between design and implementation.
- **The migration is mechanical.** `wave.name` → `wave.name()` across 27 files. Tedious but straightforward, and accessor methods keep it clean.

### Migration strategy (010): destructive reset

No existing customers and no data-retention requirement. Keep migration logic simple and correct: drop/recreate affected tables and re-seed an empty state.

`010` rebuilds wave/stimulus storage with the new invariants baked in.
`waves` is recreated with chord fields; `stimuli` is recreated with the same shape plus trigger enforcement hooks.

```sql
-- Recreate waves with chord fields and parent FK
CREATE TABLE waves (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    repo TEXT NOT NULL,
    flow TEXT NOT NULL,
    direction TEXT NOT NULL DEFAULT '[]',
    area TEXT NOT NULL DEFAULT '[]',
    paused INTEGER NOT NULL,
    status INTEGER NOT NULL,
    iteration INTEGER NOT NULL,
    created_at BIGINT NOT NULL,
    wave_type INTEGER NOT NULL DEFAULT 1,
    parent_wave_id TEXT REFERENCES waves(id) ON DELETE CASCADE,
    position INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX idx_waves_parent ON waves(parent_wave_id);
CREATE UNIQUE INDEX idx_waves_top_level_name
  ON waves(repo, name) WHERE parent_wave_id IS NULL;
CREATE UNIQUE INDEX idx_waves_child_name
  ON waves(parent_wave_id, name) WHERE parent_wave_id IS NOT NULL;
```

No backward-compat path is required; migration favors clean invariants over preserving pre-chord rows.

### Constraint strategy (DB truth + app-friendly errors)

Invariants are enforced in the DB (SQLite + Postgres), not just application code:

- Name uniqueness is enforced with partial unique indexes:
  - top-level scope: `(repo, name)` where `parent_wave_id IS NULL`
  - child scope: `(parent_wave_id, name)` where `parent_wave_id IS NOT NULL`
- Parent/stimulus mutual exclusion is enforced with DB triggers/checks:
  - deny inserting/updating `stimuli.wave_id` when target wave has `parent_wave_id IS NOT NULL`
  - deny setting `waves.parent_wave_id` when that wave already owns a stimulus

Application code catches constraint/trigger violations and returns domain errors with clear messages (`DuplicateWaveNameInScope`, `NestedWaveCannotOwnStimulus`, etc.).

### Loading strategy

Load the full subtree with one recursive CTE, then assemble in memory. No N+1 queries.

```sql
WITH RECURSIVE tree AS (
  SELECT id, parent_wave_id, position, 0 AS depth
  FROM waves
  WHERE id = ?1

  UNION ALL

  SELECT c.id, c.parent_wave_id, c.position, t.depth + 1
  FROM waves c
  JOIN tree t ON c.parent_wave_id = t.id
  WHERE t.depth < (?2 + 1) -- fetch one level beyond max to detect overflow
)
SELECT w.*, tree.depth
FROM tree
JOIN waves w ON w.id = tree.id
ORDER BY tree.depth, w.parent_wave_id, w.position;
```

```rust
const MAX_CHORD_DEPTH: u32 = 8;

async fn load_wave_tree(&self, id: &LfdId) -> StoreResult<Wave> {
    let rows = self.get_wave_subtree_rows(id, MAX_CHORD_DEPTH).await?;
    if rows.iter().any(|r| r.depth > MAX_CHORD_DEPTH) {
        return Err(StoreError::DepthLimitExceeded);
    }
    assemble_wave_tree(rows, id)
}
```

This keeps query count predictable (one query per tree load) and preserves stable child ordering by `position`.

### Voicing is not a new type

The wave plan says "make voicing explicit in the data model." The bold choice: **voicing is already explicit.** A wave's `{direction, area, flow}` fields are its voicing. When you create a chord with three children, each child's direction/area/flow is its voicing — the choices made when instantiating that voice.

Fork already does this: `merge_directions(base, extra)` combines wave directions with branch-specific ones. Chord voices inherit the same pattern: parent chord has base settings, each child overrides what it needs.

No `Voicing` struct. No `voicing` table. The concept is named in docs and API, not reified in the type system. If we later need voicing templates ("create a chord with these N voicing patterns"), that's a schema-level concept, not a data model one.

### Stimulus ownership

Stimulus ownership and parenthood are mutually exclusive:

- A wave with `parent_id != None` cannot own a stimulus.
- A wave with a stimulus must be top-level (`parent_id == None`).
- Nested waves inherit an **effective stimulus** from their nearest ancestor with a stimulus (parent or grandparent).
- Trigger propagation is inherited: one parent tick drives one execution of descendants for that tick.

This keeps `stimuli` ownership unambiguous without adding new tables.

### Failure semantics (phase 02 contract)

Chord iteration is best-effort:

- If one descendant fails, keep running other descendants for that tick.
- Wait for all started descendants to settle.
- Mark the tick as `has_failure` if any descendant failed.

This preserves maximum useful throughput while keeping failure visible at the chord level.

### Run entrypoint invariant

Nested waves cannot be run directly.

- `run_wave(child_id)` returns a domain error (e.g., `CannotRunNestedWaveDirectly`).
- Runs start from a trigger-owning top-level wave/chord.
- No implicit reroute from child to ancestor.

### Python API changes

```python
def create_chord(
    name: str,
    repo: str,
    voices: list[dict],  # each dict has name, flow, direction, area
    flow: Optional[str] = None,  # chord-level flow (optional)
    direction: Optional[list[str]] = None,
    area: Optional[list[str]] = None,
) -> Wave
```

The `voices` parameter defines the children. Each voice gets created as a Wave with `parent_id` pointing to the chord. The chord itself is a Wave::Chord.

`create_wave()` continues to create solo voices. `create_chord()` is the new entrypoint for creating chords with children.

### Naming constraint

Child wave names are scoped to their parent chord. Two chords can each have a voice named "designer", but a chord can't have two children with the same name.

```sql
-- Enforced in SQLite via partial unique indexes
-- CREATE UNIQUE INDEX idx_waves_top_level_name ON waves(repo, name) WHERE parent_wave_id IS NULL;
-- CREATE UNIQUE INDEX idx_waves_child_name ON waves(parent_wave_id, name) WHERE parent_wave_id IS NOT NULL;
```

Application code keeps pre-checks for user-friendly errors, but DB remains the source of truth.

### Migration plan for existing callsites

The ~27 files that reference Wave need mechanical updates:

| Pattern | Before | After |
|---------|--------|-------|
| Field access | `wave.name` | `wave.name()` |
| Field access | `wave.id` | `wave.id()` |
| Mutable field | `wave.status = ...` | `wave.data_mut().status = ...` |
| Construction | `Wave { id, name, ... }` | `Wave::Voice(WaveData { id, name, ... })` |
| Struct update | `Wave { name: new, ..wave }` | `wave.data_mut().name = new` |
| Type in signatures | `&Wave` | `&Wave` (unchanged) |

Most callsites don't care about the variant — they just need `data()`. The accessor methods keep these sites clean. Only code that needs to distinguish (execution, store loading) uses `match`.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Struct with `WaveKind` discriminator | No migration, existing code compiles unchanged | Children on every wave (empty vec for voices). No compiler help when chord/voice behavior diverges. The lie compounds as phases 02 and 03 add chord-specific logic. |
| Separate `chord_members` join table | Clean separation, chord membership is explicit | Extra join on every load. The parent-child relationship is simpler as a self-referential FK. Adds complexity without benefit. |
| `Voicing` struct/table | Reifies the concept | Over-engineering. direction+area+flow already ARE the voicing. Adding a type just to name something that exists is net-negative code. |
| Wave contains `Vec<WaveId>` instead of `Vec<Wave>` | Lighter in-memory, load on demand | Forces callers to do two-step lookups. The tree is small (practical max: ~10 voices). Eagerly loading children is simpler and fast enough. |

## Key decisions

1. **Enum, not struct.** Phases 02 and 03 need fundamentally different behavior for chords vs voices. The enum makes the compiler enforce exhaustive handling. The migration is mechanical (`wave.field` → `wave.field()`). Worth the upfront cost.

2. **Fail loud on invalid state.** No permissive fallback for unknown `wave_type`; decode errors and invariant violations return explicit store/domain errors.

3. **No voicing type.** "Voicing over configuration" (wave README) means the existing fields are the voicing. We name the concept in documentation and API, not in the type system. Fork drafts with different directions are already voicings — we don't need a struct to prove it.

4. **Self-referential FK, not join table.** A wave has at most one parent. The relationship is a tree, not a graph. `parent_wave_id` on the waves table is the simplest correct encoding.

5. **One-query tree loading via recursive CTE.** Avoid N+1 recursive lookups and preserve deterministic ordering. Depth limit of 8 prevents pathological nesting.

6. **Stimulus/parent mutual exclusion.** A wave can either be nested or own a trigger, never both. Nested waves inherit effective stimulus from ancestors.

7. **DB-enforced invariants + app-friendly errors.** SQLite constraints/triggers enforce correctness; API surfaces clear, stable domain errors.

8. **No direct runs for nested waves.** Running a child wave directly is invalid and returns an explicit error.

9. **Best-effort failure semantics.** One failing descendant does not halt siblings. Tick result captures failure at the aggregate level.

10. **Destructive migration is acceptable.** No existing customer data: reset tables and enforce clean constraints immediately.

## Scope

In scope:
- `Wave` enum (Voice | Chord) and `WaveData` struct
- Accessor methods on `Wave` for common fields
- `parent_id` and `position` fields on WaveData
- Migrate ~27 files from struct field access to accessor methods
- SQLite migration 010
- Store methods: `load_wave_tree` (CTE-backed), `create_chord`
- Python API: `create_chord()` function
- Round-trip tests for solo voice, chord with voices, nested chord

Out of scope:
- Execution changes (phase 02)
- Listen step (phase 03)
- Rhythm variant (future)
- CLI commands for chord management
- Schema-based chord creation (templates)
- Model as a voicing parameter (stays at step/agent level)

## Done when

```bash
# Rust tests pass
cargo test -p loopflow wave
cargo test -p loopflow chord

# Specific verification:
# 1. Wave is an enum with Voice(WaveData) and Chord { data, children } variants
# 2. Accessor methods (id(), name(), repo(), data(), data_mut(), children()) exist
# 3. All existing callsites compile with accessor methods
# 4. Migration 010 rebuilds wave/stimulus schema with new constraints
# 5. Create a chord with 2 voice children, persist, reload — structure matches
# 6. Create a nested chord (chord containing a chord), persist, reload — structure matches
# 7. Existing solo waves load unchanged as Wave::Voice with no parent
# 8. Depth limit rejects loading beyond 8 levels
# 9. Python create_chord() creates parent + children in one call
# 10. Nested waves cannot own stimuli; top-level waves can
# 11. Nested chord executes once per parent trigger tick (no independent child scheduling)
# 12. Name uniqueness is DB-enforced for top-level and child scopes
# 13. Running nested wave directly returns explicit error (no auto-reroute)
# 14. On descendant failure, siblings still run and tick is marked failed
# 15. cargo clippy -- -D warnings passes
# 16. cargo fmt passes
```
