# 05: Beat Grid

Sequenced execution for chords. A beat grid defines which children fire on which beat, turning a chord into a drum machine.

## What exists after this

A chord can optionally have a beat grid — a sequence of beats where each beat toggles children on/off. Without a grid, all children fire every tick (current behavior). With a grid, the chord plays through beats in order: fire active children in parallel, wait for all to complete, advance to next beat. The full grid plays as one tick from the parent's perspective.

## Design decisions

**Beat grid is metadata on Chord, not a new Wave variant.** A Chord already has children. The grid is an execution strategy — it says *when* each child fires, not *what* the children are. No new enum variant needed.

```rust
enum Wave {
    Voice(WaveData),
    Chord { data: WaveData, children: Vec<Wave>, beats: Option<Vec<Vec<bool>>> },
}
// beats[beat_index][child_index] = active
```

**No grid = all-on every tick.** Backwards compatible. Existing chords behave exactly as before.

**Single-child beat grids are valid.** A chord with one child and a beat grid is a repeat loop — run the same voice N times in sequence.

**No silent beats.** Every beat must have at least one active child. A fully silent beat is wasted execution. Invariant: `beat.active.iter().any(|&on| on)` for every beat.

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

With beat grids, nesting becomes meaningful — a nested chord on a beat acts as one unit that plays its own sequence before yielding. Phase 01 shipped the `nest` parameter on `join`:

- `join(a, b)` — absorb (flat, merges B's children into A)
- `join(a, b, nest=true)` — nest B as a child of A, keeping B's children intact

The beat grid makes nesting useful because you can sequence when nested sub-groups fire. The `nest` parameter is already in the store, HTTP, and Python API — no additional data model work needed for phase 04.

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
ALTER TABLE waves ADD COLUMN beats TEXT;  -- JSON: [[true, false, true], [false, true, false], ...]

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
    [True,  False, True ],  # beat 1: designer + reviewer
    [False, True,  False],  # beat 2: infra only
    [True,  True,  False],  # beat 3: designer + infra
    [False, False, True ],  # beat 4: reviewer only
])

# Clear the grid (back to all-on)
loopflow.set_beats("ensemble", None)
```

```
PUT /v0/waves/{wave_id}/beats   { "beats": [[true, false], [false, true]] }
DELETE /v0/waves/{wave_id}/beats
```

## Phases

| # | Focus | Scope |
|---|-------|-------|
| 1 | Storage + API | beats column, set/clear endpoints, validation |
| 2 | Execution | sequential beat playback in the chord executor |
| 3 | UI | drum machine grid view in Concerto |

## Invariants

- Every beat has at least one `true` (no silent beats)
- `beats[i].len() == children.len()` for all `i` (grid width matches child count)
- Single-child beat grids are valid (repeat semantics)
- Children can be any Wave variant (Voice, Chord, nested Chord with its own beats)

## Open questions

- Should beats have names/labels? ("setup", "build", "review" vs just beat 1, 2, 3)
- Can a beat have per-voice overrides? (different flow/direction for a voice on a specific beat)
- Should the grid auto-resize when children are added/removed via join?
- Looping: does the grid always loop, or can it play once and stop?
- Tempo: is every beat one tick, or can beats have different durations?
- How does `join` interact with BeatGrid? Does joining a voice into a beat grid auto-extend all beats with a new `false` slot? (The `nest` parameter is resolved — `join(a, b, nest=true)` handles chord-into-chord nesting. The grid resize question remains.)

## Done when

- Chords can have optional beat grids defining per-beat voice activation
- Beat grids play sequentially, firing active children in parallel per beat
- No beat is fully silent
- Full grid completion counts as one tick from the parent's perspective
- Nesting works: a beat containing a chord plays the child chord's full sequence
- Single-voice beat grids work as repeat loops
