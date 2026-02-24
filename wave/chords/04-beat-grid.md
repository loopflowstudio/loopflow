# 04: Beat Grid

Sequenced execution for chords. A beat grid defines which children fire on which beat, turning a chord into a drum machine.

## What exists after this

A chord can optionally have a beat grid — a sequence of beats where each beat toggles children on/off. Without a grid, all children fire every tick (current behavior). With a grid, the chord plays through beats in order: fire active children in parallel, wait for all to complete, advance to next beat. The full grid plays as one tick from the parent's perspective.

## Design decisions

**Beat grid is metadata on Chord, not a new Wave variant.** A Chord already has children. The grid is an execution strategy — it says *when* each child fires, not *what* the children are. No new enum variant needed.

```rust
enum Wave {
    Voice(WaveData),
    Chord { data, children, beats: Option<Vec<Beat>> },
}

struct Beat {
    active: Vec<bool>,  // positional, one per child
}
```

**No grid = all-on every tick.** Backwards compatible. Existing chords behave exactly as before.

**The full grid plays as one unit.** A 4-beat grid with 3 voices: the chord runs 4 sequential steps, each step firing the active voices in parallel. When all 4 beats complete, the chord signals "done" to its parent. From the parent's perspective, the grid is one atomic execution — like a single voice that happens to take longer.

**Nesting composes naturally.** A beat grid containing a nested chord (or another beat grid) treats it as one child. When that child's beat is active, the nested chord/grid plays its full sequence before the parent advances. Beat grids inside beat grids work recursively.

**Children include nested chords.** "Voice" in the beat grid UI means "child of this chord" — it could be a Voice wave, a Chord, or a Chord with its own beat grid. The grid doesn't care about the child's internal structure.

## The drum machine analogy

```
         beat 1    beat 2    beat 3    beat 4
designer   ●         ○         ●         ○
infra      ○         ●         ○         ●
reviewer   ○         ○         ○         ●
```

Each row is a child wave. Each column is a beat. Toggle cells on/off. The chord plays left to right, then loops.

The UI is a grid. Click to toggle. Add/remove beats. Reorder voices by dragging. This is the "sequence view" of a chord.

## Nesting and join

With beat grids, nesting becomes meaningful — a nested chord on a beat acts as one unit that plays its own sequence before yielding. This means `join` should support both flat absorption and nesting:

- `join(a, b)` — absorb (current behavior, flat)
- Explicit nesting operation (TBD) — add a chord as a child of another chord without flattening

The beat grid makes nesting useful because you can sequence when nested sub-groups fire.

## Execution semantics

1. Chord tick fires (from stimulus or parent beat)
2. If no beat grid: fire all children in parallel, wait for all, done
3. If beat grid: for each beat in sequence:
   a. Fire all active children for this beat in parallel
   b. Wait for all active children to complete
   c. If any child fails, record failure but continue to next beat (best-effort)
   d. Advance to next beat
4. After all beats complete, the chord tick is done
5. If the chord has a looping stimulus, advance iteration counter and restart from beat 1

## Storage

Beat grid stored as JSON in the waves table or a separate `beat_grids` table:

```sql
-- Option A: column on waves table
ALTER TABLE waves ADD COLUMN beats TEXT;  -- JSON: [{"active": [true, false, true]}, ...]

-- Option B: separate table
CREATE TABLE beat_grids (
    wave_id TEXT PRIMARY KEY REFERENCES waves(id) ON DELETE CASCADE,
    beats TEXT NOT NULL  -- JSON array of beat objects
);
```

Option A is simpler. The column is NULL for voices and chords without grids.

## API

```python
# Set a 4-beat grid on an existing chord
loopflow.set_beats("ensemble", [
    {"active": [True, False, True]},   # beat 1: designer + reviewer
    {"active": [False, True, False]},  # beat 2: infra only
    {"active": [True, True, False]},   # beat 3: designer + infra
    {"active": [False, False, True]},  # beat 4: reviewer only
])

# Clear the grid (back to all-on)
loopflow.set_beats("ensemble", None)
```

```
PUT /v0/waves/{wave_id}/beats   { "beats": [...] }
DELETE /v0/waves/{wave_id}/beats
```

## Phases

| # | Focus | Scope |
|---|-------|-------|
| 1 | Storage + API | beats column, set/clear endpoints, validation |
| 2 | Execution | sequential beat playback in the chord executor |
| 3 | UI | drum machine grid view in Concerto |

## Open questions

- Should beats have names/labels? ("setup", "build", "review" vs just beat 1, 2, 3)
- Can a beat have per-voice overrides? (different flow/direction for a voice on a specific beat)
- Should the grid auto-resize when children are added/removed via join?
- Looping: does the grid always loop, or can it play once and stop?
- Tempo: is every beat one tick, or can beats have different durations?

## Done when

- Chords can have optional beat grids defining per-beat voice activation
- Beat grids play sequentially, firing active children in parallel per beat
- Full grid completion counts as one tick from the parent's perspective
- Nesting works: a beat grid containing a chord plays the child chord's full sequence
- Default behavior (no grid) is unchanged
