---
asana_id: '1213718347017619'
linear_id: ed3c82de-b25b-4def-8ca7-c7dd09d591ad
notion_id: 32af8f99-3d81-81ee-bb97-ddf1d3b929d9
---
# 07: DAG and Nested Chords

**Finish line:** Chord-waves can contain other chord-waves, forming a DAG. Acyclicity is enforced. Worker capacity budgets flow down the tree correctly.

## Context

The redesign chord-wave already proves the first level: a wave can coordinate other waves by pointing its `area` at their directories. The remaining question is how far that model stretches. This item validates nesting and uses the same area-derived membership model to introduce a default top-level chord-wave.

Worker capacity (`workers: N`) now flows as a budget down the chord tree. Intermediate waves split their budget among children and themselves — e.g., an `api` wave with 6 workers keeps 1 for cross-cutting work and distributes 3 to `api-auth` and 2 to `api-billing`. This is a policy decision (s5-level, slow-changing), not recomputed each cycle. The root wave is not special — it's just the wave at the top of the tree. Any wave with children can run the planning flow.

## What to build

1. **DAG validation.** When a wave adds another `wave/<name>/` directory to `area`, treat that as an edge in the chord graph. If the target is itself a chord-wave, walk the implied graph and reject cycles.

2. **Default chord-wave creation.** `lfq init` or equivalent creates a default chord-wave for a repo and seeds its `area` with the existing waves.

3. **Restructuring through tend.** The default chord-wave's first tend cycle reads the existing waves, compares them against the redesign direction, and proposes restructuring for human review instead of requiring manual wave surgery.

4. **Nested UI support.** Concerto can render nested chord-waves so the default chord-wave sits above project chord-waves and leaf waves.

## Done when

- Adding a chord-wave as a member of another chord-wave works
- Cycle detection rejects invalid membership
- A default chord-wave can be created and absorb existing waves
- Its first tend cycle can propose restructuring
- A human can approve or reject the restructuring proposal
