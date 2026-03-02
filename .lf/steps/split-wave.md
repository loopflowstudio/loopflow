---
requires: wave/<wave>/README.md
produces: wave/<child>/ directories
---
Decompose a wave into smaller, independent waves. The original wave is replaced entirely.

## Goal

Wave mitosis. The parent wave ceases to exist — its content is distributed across N new waves.

The numeric argument controls how many children to create (default 2).

Sprint files move as-is — each one lands in exactly one child. But the README sections need rewriting, not slicing:

- **Vision**: written fresh for each child. Must be internally coherent, not a fragment of the parent's.
- **Goals**: scoped to each child's slice of the work.
- **Risks**: reassessed per child. Some parent risks won't apply; new ones may emerge.
- **Metrics**: preserved and distributed. A metric can appear in multiple children if it spans both.

## Workflow

1. Read the parent wave
   - Use the wave passed by argument, or ask which `wave/<name>/` to split
   - Read the README and all numbered roadmap files

2. Find split boundaries
   - Look for thematic clusters, dependency chains, or independent workstreams
   - Aim for the requested count (default 2)
   - Each resulting wave should stand alone

3. Allocate sprints
   - Assign each sprint to exactly one child — no orphans

4. Create the new waves
   - `wave/<child>/README.md` — fresh Vision and Goals for each child; Risks and Metrics carried forward and adapted
   - `wave/<child>/<child>.yaml` — flow, area, optional direction/triggers
   - Numbered sprint files from the allocated items, each with a clear finish line
   - Use `### Not here` under Vision to draw boundaries between siblings

5. Remove the parent
   - Delete `wave/<parent>/`
   - Commit: `split-wave: <parent> → <child-a>, <child-b>`

6. Verify
   - Each child has a README, a matching YAML, and at least one sprint file
   - No content from the parent is unaccounted for

## Guardrails

- Carry forward the original wave's intent — don't reshape the product direction during a split
- Concrete, domain-specific names for children
- When a boundary is unclear, pick the simpler option and note the alternative in `scratch/questions.md`
