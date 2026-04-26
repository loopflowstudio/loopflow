---
asana_id: '1213883255344326'
linear_id: ed3c82de-b25b-4def-8ca7-c7dd09d591ad
notion_id: 32af8f99-3d81-81ee-bb97-ddf1d3b929d9
---
# DAG and nested garden waves

**Finish line:** A wave whose `area` includes other `wave/` directories can itself be a member of another such wave, forming a DAG. Acyclicity is enforced. Worker capacity budgets flow down the tree correctly.

## Context

The current root wave already proves the first level: a wave can coordinate other waves by pointing its `area` at their directories. The remaining question is how far that model stretches. This item validates nesting and introduces a safe default root wave for repos that do not have one yet.

Worker capacity (`workers: N`) flows as a budget down the tree. Intermediate waves split their budget among children and themselves. The root wave is not special — it is just the wave at the top of the tree. Any wave with children can run the planning flow.

## What to build

1. **DAG validation.** When a wave adds another `wave/<name>/` directory to `area`, treat that as an edge in the coordination graph. If the target also coordinates waves, walk the implied graph and reject cycles.
2. **Default root creation.** `lfq init` or equivalent creates a default root wave for a repo and seeds its `area` with the existing waves.
3. **Restructuring through garden.** The default root wave's first garden cycle reads the existing waves, compares them against the desired shape, and proposes restructuring for human review instead of requiring manual wave surgery.
4. **Nested UI support.** Concerto can render nested coordinating waves so the default root can sit above project-level waves and leaf waves.

## Done when

- Adding a coordinating wave as a member of another coordinating wave works
- Cycle detection rejects invalid membership
- A default root wave can be created and absorb existing waves
- Its first garden cycle can propose restructuring
- A human can approve or reject the restructuring proposal
