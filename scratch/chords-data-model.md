# Chords Data Model

## Problem

Waves are solo. You can fork a wave's flow (run the same step with 3 different directions), but fork branches are ephemeral — they live in temp worktrees, execute, synthesize, and disappear. There's no persistence across iterations, no awareness between voices, no hierarchy.

Chords make waves compositional. A chord is a wave that contains other waves. The children persist, iterate, and (in phase 03) listen to each other. This is the foundation for multi-agent collaboration at the wave level.

## Approach

Extend the existing `Wave` struct with three fields: `wave_type`, `parent_id`, and a transient `children` vec. No enum. No new `WaveData` type. The existing Wave fields (direction, area, flow) already ARE the voicing parameters — we name the concept, not add new columns.

### Wave struct changes

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum WaveKind {
    #[default]
    Voice = 1,
    Chord = 2,
}

pub struct Wave {
    // ... existing fields unchanged ...
    pub kind: WaveKind,                  // voice or chord
    pub parent_id: Option<LfdId>,        // null for solo waves and top-level chords
    pub position: u32,                   // ordering within parent chord (0 for solo)
    #[serde(default)]
    pub children: Vec<Wave>,             // populated on load for chords, empty for voices
}
```

Why struct, not enum:
- Wave is referenced in ~20+ files across types, store, executor, HTTP routes, Python API. An enum means every field access becomes a match or accessor.
- A chord IS a wave. Same fields. The discriminator is whether it has children.
- Existing code that doesn't care about children keeps compiling unchanged.
- The enum can be introduced later if type safety proves necessary. The migration is mechanical.

### SQLite migration (010)

```sql
ALTER TABLE waves ADD COLUMN wave_type INTEGER NOT NULL DEFAULT 1;
ALTER TABLE waves ADD COLUMN parent_wave_id TEXT REFERENCES waves(id) ON DELETE CASCADE;
ALTER TABLE waves ADD COLUMN position INTEGER NOT NULL DEFAULT 0;

CREATE INDEX idx_waves_parent ON waves(parent_wave_id);
```

All existing waves become voice (wave_type=1) with no parent. Fully backwards-compatible. No data migration needed.

### Loading strategy

Single query to get children, recursive up to depth limit:

```rust
const MAX_CHORD_DEPTH: u32 = 8;

async fn load_wave_tree(&self, id: &LfdId, depth: u32) -> StoreResult<Wave> {
    if depth > MAX_CHORD_DEPTH {
        return Err(StoreError::DepthLimitExceeded);
    }
    let mut wave = self.get_wave(id)?;
    if wave.kind == WaveKind::Chord {
        let children = self.get_children(id)?;
        for mut child in children {
            if child.kind == WaveKind::Chord {
                child = self.load_wave_tree(&child.id, depth + 1)?;
            }
            wave.children.push(child);
        }
    }
    Ok(wave)
}

fn get_children(&self, parent_id: &LfdId) -> StoreResult<Vec<Wave>> {
    // SELECT * FROM waves WHERE parent_wave_id = ? ORDER BY position
}
```

Practical nesting is 2-3 levels. Depth limit of 8 is generous without being dangerous.

### Voicing is not a new type

The wave plan says "make voicing explicit in the data model." The bold choice: **voicing is already explicit.** A Wave's `{direction, area, flow}` fields are its voicing. When you create a chord with three children, each child's direction/area/flow is its voicing — the choices made when instantiating that voice.

Fork already does this: `merge_directions(base, extra)` combines wave directions with branch-specific ones. Chord voices inherit the same pattern: parent chord has base settings, each child overrides what it needs.

No `Voicing` struct. No `voicing` table. The concept is named in docs and API, not reified in the type system. If we later need voicing templates ("create a chord with these N voicing patterns"), that's a schema-level concept, not a data model one.

### Stimulus ownership

Stimuli already have a `wave_id` foreign key. A chord's stimulus belongs to the chord wave. Child voices don't have their own stimuli — they fire when the chord fires.

This works with the existing `stimuli` table unchanged. The chord's `wave_id` in the stimuli table is the chord's ID. Phase 02 (execution) handles the "start all children when chord fires" logic.

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

The `voices` parameter defines the children. Each voice gets created as a Wave with `parent_id` pointing to the chord. The chord itself is also a Wave with `kind=Chord`.

`create_wave()` continues to create solo voices. `create_chord()` is the new entrypoint for creating chords with children.

### Naming constraint

Child wave names are scoped to their parent chord. Two chords can each have a voice named "designer", but a chord can't have two children with the same name.

```sql
-- Enforced in application code, not SQL constraint
-- (SQLite's UNIQUE(name, repo) already exists for top-level waves)
```

For children, uniqueness is (parent_wave_id, name). Enforced in `create_wave`/`create_chord` before insert.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Wave enum (Voice/Chord variants) | Type-safe pattern matching | Touches every file that accesses Wave fields. Massive refactor for a discriminator that can be a field. Enum is the right endpoint but wrong starting point. |
| Separate `chord_members` join table | Clean separation, chord membership is explicit | Extra join on every load. The parent-child relationship is simpler as a self-referential FK. Adds complexity without benefit. |
| `Voicing` struct/table | Reifies the concept | Over-engineering. direction+area+flow already ARE the voicing. Adding a type just to name something that exists is net-negative code. |
| Wave contains `Vec<WaveId>` instead of `Vec<Wave>` | Lighter in-memory, load on demand | Forces callers to do two-step lookups. The tree is small (practical max: ~10 voices). Eagerly loading children is simpler and fast enough. |

## Key decisions

1. **Struct, not enum.** "Chords are waves" (wave README) — literally. Same struct, extra fields. The wave plan's enum is the right concept but wrong encoding for a system where Wave is used as a flat struct everywhere. We get the same semantics with a `WaveKind` discriminator field.

2. **No voicing type.** "Voicing over configuration" (wave README) means the existing fields are the voicing. We name the concept in documentation and API, not in the type system. Fork drafts with different directions are already voicings — we don't need a struct to prove it.

3. **Self-referential FK, not join table.** A wave has at most one parent. The relationship is a tree, not a graph. `parent_wave_id` on the waves table is the simplest correct encoding.

4. **Eager tree loading with depth limit.** Chords have few children (2-5 typical, 10 max practical). Loading the full tree is fast and keeps the API simple. Depth limit of 8 prevents pathological nesting.

5. **Additive migration only.** Three new columns with defaults. Existing waves untouched. No data migration. Rollback is dropping the columns.

## Scope

In scope:
- `WaveKind` enum (Voice, Chord) and field on Wave struct
- `parent_id` and `position` fields on Wave
- `children` transient field for loaded tree
- SQLite migration 010
- Store methods: `load_wave_tree`, `get_children`, `create_chord`
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
cargo test -p loopflow wave_kind
cargo test -p loopflow chord

# Specific verification:
# 1. Wave struct has kind, parent_id, position, children fields
# 2. Migration 010 adds columns to waves table
# 3. Create a chord with 2 voice children, persist, reload — structure matches
# 4. Create a nested chord (chord containing a chord), persist, reload — structure matches
# 5. Existing solo waves load unchanged (kind=Voice, no parent, no children)
# 6. Depth limit rejects loading beyond 8 levels
# 7. Python create_chord() creates parent + children in one call
# 8. cargo clippy -- -D warnings passes
# 9. cargo fmt passes
```
