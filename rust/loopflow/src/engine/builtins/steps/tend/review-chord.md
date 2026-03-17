---
requires: scratch/tend-chord.md
produces: scratch/tend-chord.md (annotated)
interactive: true
---
Review the proposed chord with the human. Decide which mutations land.

## Goal

The chord is a set of proposed mutations to member waves. The human decides
which ones to apply, which to defer, and which to reject. This is the
highest-leverage moment in the tend cycle — small decisions here reshape
how all waves work going forward.

## Opening

Orient the human. They're context-switching into a meta view of their waves.

1. **Chord summary** — how many mutations, which waves are affected, the
   overall thrust. One paragraph.
2. **Assessment highlights** — the pressure points that motivated these
   mutations. Link back to specific observations.

Don't editorialize. Present the chord, let the human react.

## Walkthrough

Present each mutation. For each:

- Show the before/after concretely
- Explain the rationale without overselling
- Flag risks honestly
- Ask for a verdict: **apply**, **defer**, or **reject**

Pause after each mutation. Let the human think. If they want to modify
a mutation rather than accept or reject it wholesale, work with them to
revise it in place.

## Cross-Cutting Questions

After individual mutations, zoom out:

- **Trajectory** — Are we making real progress toward the chord-wave's goals?
  Or grinding on details that don't compound?
- **Surprise** — Did the scan or assessment surface anything the human didn't
  expect? Surprises are information — explore them.
- **Missing** — Is there a mutation the human expected to see but didn't?
  Or a pressure they feel that the assessment missed?
- **Silent waves** — Show which waves are silent and why. Each is an
  invitation: the human can add items to any silent wave to direct work
  there. This is how human intent flows into the system — not by
  overriding the chord, but by seeding a wave's backlog.
- **Calibration** — Does the human want to adjust the tend process itself?
  Different scan depth, different assessment criteria, different mutation style?

## Output

Update `scratch/tend-chord.md` with verdicts:

```markdown
### 1. <title>
...
**Verdict**: apply | defer | reject
**Notes**: <human's reasoning or modifications, if any>
```

Add a section at the end:

```markdown
## Session Notes
<Anything the human said that should inform future tend cycles.
Trajectory observations, calibration adjustments, context that
Letta should remember.>
```

## What to avoid

**Rubber-stamping.** If the human approves everything without discussion,
something is wrong — either the mutations are too safe or the review isn't
surfacing the real tensions. Push gently on at least one.

**Defending mutations.** Present, don't advocate. If the human rejects a
mutation, understand why. Their reasoning is information for the next cycle.

**Rushing.** This step is interactive for a reason. The human's attention
is the scarcest resource. Use it well.
