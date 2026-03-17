Judge the health of each wave and the chord as a whole. Where is momentum? Where is drift?

## Goal

The scan is raw data. This step turns it into judgment. Which waves are
thriving? Which are stalled? Is the overall direction still right, or has
the ground shifted?

Assessment is honest. A wave that's producing PRs but shipping shallow work
isn't healthy. A wave that's blocked but blocked on the right problem might
be fine. Look past activity to actual progress toward finish lines.

## Workflow

1. **Read the scan.** `scratch/tend-scan.md` is your primary input.

2. **Assess each wave.** For each member wave, evaluate:

   - **Velocity** — Is work shipping? At what rate? Accelerating or decelerating?
   - **Depth** — Is the work substantive? Are finish lines actually being crossed,
     or is the wave producing motion without progress?
   - **Alignment** — Is the wave building what its README says it should?
     Has the work drifted from the stated goals?
   - **Health** — Are blocks being resolved or accumulating? Is CI green?
     Are PRs aging?
   - **Sequencing** — Is the wave working on the highest-leverage item?
     Would reordering the remaining items change outcomes?
   - **Silence** — A silent wave has no items, or its items didn't survive
     coherence review. That's healthy when nothing compelling exists to build.
     It's a problem when the area is changing and the wave isn't noticing.
     Silent waves signal to the human: "add items here if you want work
     done in this area."
   - **Coherence** — Do the wave's remaining items still make sense? The
     codebase evolves between tend cycles. Items can go stale: finish lines
     moved, designs diverged, value diminished. Waves should reorganize
     internally — this is a single beat, not a human review. Flag waves
     whose items look incoherent so draft-chord can account for it.

3. **Assess the chord.** Look across all waves:

   - **Balance** — Are waves progressing roughly in sync, or has one
     raced ahead while others stall?
   - **Interference** — Are waves stepping on each other? Conflicting
     changes, competing for the same code, contradictory directions?
   - **Gaps** — Is there work that needs doing that no wave owns?
   - **Redundancy** — Are multiple waves doing equivalent work?
   - **Phase fit** — Does the current phase (from the roadmap) still
     make sense given what's been learned?

4. **Identify the pressure points.** What are the 1-3 things that, if
   changed, would have the biggest impact on overall progress?

## Output

Write `scratch/tend-assessment.md`:

```markdown
# Tend Assessment — <date>

## Summary
<2-3 sentences: overall chord health and the key tension>

## Wave: <name>
**Health**: thriving | steady | drifting | stalled | blocked | silent
**Evidence**: <specific observations from the scan>
**Pressure**: <the one thing most affecting this wave's progress>

(repeat for each member wave)

## Chord-Level
**Balance**: <how waves relate to each other>
**Gaps**: <unowned work, if any>
**Phase**: <is the current roadmap phase still right?>

## Pressure Points
1. <highest leverage change>
2. <second>
3. <third, if needed>
```

## What to avoid

**Vague health ratings.** "Steady" without evidence is useless. Every judgment
needs a specific observation backing it.

**Activity as proxy for progress.** Lots of commits doesn't mean a wave is
healthy. Finish lines crossed is the metric.

**Premature solutions.** Identify pressure points, don't propose fixes. That's
draft-chord's job.


## Routing

After completing your analysis, choose one of these paths:

- **chord**: Pressure points found — compose and review mutations
- **reorg**: No cross-wave pressure, but individual waves need coherence passes
- **silence**: Everything is healthy — no action needed this cycle

Write your choice to `scratch/route-or.md`.
First line must be exactly: `path: <key>`
Then explain your reasoning briefly.
