# Detail View Density and Hierarchy

Polish item from `roadmap/viz/07-polish.md` "Other candidates".

## Problem

WaveDetailPanel is the most complex view in Concerto (~690 lines). After Tier 5 cleanup (Label extraction, `perform` helper, spacing tokens), the code structure is solid. The remaining density/hierarchy issues are visual:

1. **Flat section hierarchy.** Commits, Diff, Live Output, and Ops Actions all sit at the same visual level. Nothing guides the eye to what matters most for the current state.

2. **Section headers are heavy.** "Commits", "Diff", "Live Output" use `Typography.caption()` + `.fontWeight(.medium)` + `.secondary` — same weight as sidebar section headers were before Tier 5. These are labels, not destinations.

3. **Ops actions bar competes with content.** The Land/Next/View PR bar uses `palette.surface` background + rounded corners, giving it card-level prominence when it should feel like inline tooling.

4. **Config summary (running state) is sparse.** Three `configLabel` items in an `HStack` — area, direction, flow — with no container. Floats between the header and progress section.

## Candidates

Evaluate each against visual impact per line changed. Small diffs only.

### A. Subdue section headers further
Match the Tier 5 sidebar pattern: lighter opacity, smaller size. "Commits" and "Diff" are reference info, not primary content.

### B. Group related sections into cards
Commits + Diff could share one card (git state). Ops actions could be an inline toolbar at the bottom of that card rather than its own floating bar.

### C. Running-state config summary container
Wrap area/direction/flow in a subtle card to visually anchor it, matching the progress section card below it.

### D. Tighten vertical spacing between sections
`Spacing.lg` (16pt) between all sections may be too generous for closely related content. Commits and Diff are tightly coupled — `Spacing.sm` (8pt) between them, `Spacing.lg` before the next conceptual group.

## Constraints

- 20-200 lines changed per PR
- No model or state changes
- Must work for all wave states (idle, running, failed, waiting)
- Design tokens only — no new literal values
