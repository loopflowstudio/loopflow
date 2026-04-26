---
notion_id: 32af8f99-3d81-8123-89d7-c8fb5d86549c
---
# Beat synthesizer

**Finish line:** A Concerto view where you program a coordinating wave's rhythm — assign member waves to beat slots in a sequencer grid, visualize the pattern, and run it. The garden flow is the downbeat.

## The view

A sequencer grid. Rows are member waves. Columns are beats in a cycle. Each cell is either empty (silent) or filled (that wave runs on that beat).

```text
beat:       1      2      3      4      5
root:       ■
desktop:           ■      ■             ■
mobile:                   ■      ■
workflows:        ■             ■      ■
```

Beat 1 is garden — the coordinating wave runs its own pass. The other beats are builds on specific waves. Silent waves stay visible but grayed out.

## Interaction (v1)

- Pick the number of beats and generate a default grid
- Show a moving playhead while the cycle is running
- Keep the first version read-only: visualize what is scheduled on each beat
- Keep silent waves visible so it is obvious what is not running

The full grid editor comes later. The data model supports arbitrary per-beat wave assignments from day one — power users can edit the YAML directly or root's garden flow can propose grid mutations.

## What this enables

More beats mean more autonomy between garden check-ins. Fewer beats mean tighter feedback. Removing a wave from beats staggers work. Adding garden beats increases oversight. The grid makes silence visible.

## Data model

```yaml
# wave/root/root.yaml
flow: garden
mode: manual
beats: 5
grid:
  1: [root]
  2: [desktop, workflows]
  3: [mobile, desktop]
  4: [mobile, workflows]
  5: [desktop, workflows]
```

`beats:` and `grid:` are the raw data model. Presets can generate them, but the stored form stays simple YAML.

## Relationship to silence

The synthesizer makes silence visible. A wave with no beats is silent — it appears in the sidebar, grayed out, available but not scheduled. Adding it to a beat wakes it up. Clearing all of its beats silences it.
