---
requires: scratch/ux-research/loop-NN/proposal.md
produces: scratch/ux-research/loop-NN/candidates.md
---
Prototype **at least three genuinely-distinct design candidates** for the
behavior in this loop's proposal.

## The bar for "distinct"

Variations of one idea don't count. Candidates must differ in a load-bearing
dimension — the *default view*, the *primary navigation*, the *interaction
model*, or *what the screen is organized around*. If you could get from one
candidate to another by moving a button, they're the same candidate.

Force the spread. Deliberately include approaches that pull in opposite
directions (e.g. a visual dashboard vs. a keyboard-first zero-UI path), so the
evaluation surfaces real tension a human has to resolve.

## Ground each candidate

- Describe a concrete screen: layout, what's on it, and the interaction to
  perform the target behavior. **ASCII / wireframe sketches are expected.**
- Reference real SwiftUI structure where it helps (e.g. "keep
  `NavigationSplitView` but…", "replace the `List` with a `LazyVGrid`…"),
  and real model fields the design would surface (`waitingReason`, `iteration`,
  `diffStat`, `openPRCount`, `trigger`).
- Respect the guardrails from `waves-one-level-out.md`: frame don't render
  (the list routes attention; the *wave screen* hosts the terminal), repo is a
  filter, GOAL/MEMORY is singular identity. A candidate may *challenge* a
  guardrail — say so explicitly and let evaluation judge it.
- Note what the candidate bets on (the hypothesis) and what it sacrifices.

## Output

`scratch/ux-research/loop-NN/candidates.md`:

```markdown
# Loop NN — Design Candidates

## Candidate A — <name>
**Bet:** <the hypothesis this design is testing>
**Sketch:**
<ascii wireframe>
**How the behavior plays out:** <step by step>
**Surfaces / SwiftUI:** <model fields shown; structural note>
**Sacrifices:** <what it gives up>

## Candidate B — <name>
...

## Candidate C — <name>
...
```
