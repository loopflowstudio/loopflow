---
asana_id: '1213718096104795'
linear_id: 197904a5-23aa-41c2-afc4-edc1b078d2e5
---
# 04: Beat Synthesizer

**Finish line:** A Concerto view where you program a chord's rhythm — assign waves to beat slots in a sequencer grid, visualize the pattern, and run it. The chord's own tend flow is the downbeat.

## The View

A sequencer grid. Rows are waves (member waves of a chord). Columns are beats (numbered slots in a cycle). Each cell is either empty (silent) or filled (that wave runs on that beat).

```
beat:     1      2      3      4      5      6      7      8
tend:     ■                    ■
deck:            ■                    ■
chord:                  ■                    ■
signal:                        ■                    ■
embed:                                              ■      ■
```

Beat 1 and 5 are tend — the chord itself runs, scanning and assessing. The other beats are builds on specific waves. Silent waves have no filled cells.

The number of beats is configurable per chord (default: 8). The pattern repeats — after beat 8, back to beat 1.

## Interaction (v1)

- Pick number of beats — generates default grid (tend + all active waves)
- Running state shows a playhead moving across the beats
- Read-only grid visualization — see what's scheduled on each beat
- Silent waves visible but grayed out

The full grid editor (drag waves into cells, click to toggle) comes later.
The data model supports arbitrary per-beat wave assignments from day one —
power users can edit the YAML directly or the chord's tend flow can
propose grid mutations.

## What this enables

More beats = more autonomy between tend check-ins. Fewer beats = tighter
feedback. Removing waves from beats staggers work. Adding tend beats
increases oversight. The grid is direct manipulation — what you see is
what runs.

## Rhythms

A rhythm like `4:1` means 4 build beats per tend beat — a 5-beat grid.
`3:1` is a 4-beat grid. The ratio directly determines the cycle length.

```
4:1 with 3 active waves:

beat:     1      2      3      4      5
          tend   all    all    all    all
                 ───    ───    ───    ───
                 chord  chord  chord  chord
                 embed  embed  embed  embed
```

The default fill is all active waves on every build beat. Edit from
there — remove waves from beats to stagger, add tend beats for more
oversight, clear all beats for a wave to silence it.

## Data model

```yaml
# wave/redesign/redesign.yaml
flow: tend
mode: loop
beats: 5
grid:
  1: [tend]
  2: [chord-model, agent-embedding]
  3: [chord-model, agent-embedding]
  4: [chord-model, agent-embedding]
  5: [chord-model, agent-embedding]
```

`beats:` and `grid:` are the raw data model. A rhythm like `4:1` is a
preset that generates them — 5 beats, tend on 1, all active waves on
2–5. The preset writes `beats:` and `grid:` to YAML; after that it's
just data you can edit directly.

The grid is a full matrix — any number of waves per beat, any number of
beats per wave. Each wave on a given beat gets one flow run (one PR cycle).
Waves on the same beat run in parallel. `tend` on a beat means the chord's
own tend flow runs.

## Relationship to silence

The synthesizer makes silence visible. A wave with no beats is silent — it appears in the sidebar, grayed out, available but not scheduled. Dragging it into a beat slot is how you wake it. Clearing all its beats is how you silence it.

This is the same silence concept from the roadmap, but with a concrete UI. The blocking queue shrinks because you can see exactly which waves are competing for attention and adjust.
