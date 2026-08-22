---
requires: wave/<wave>/GOAL.md
produces: wave/<child>/ directories
---
Decompose a wave into smaller, independent waves. The original wave is replaced entirely.

## Orientation

Before starting, orient yourself in this branch:

- Read `scratch/` — design docs and notes for the current work live here
  (`scratch/<branch>.md` is this PR's design; `scratch/questions.md` holds open
  questions and assumptions).
- If a `wave/<name>/` directory matches this work, skim its `GOAL.md`/`MEMORY.md`,
  PM snapshot, and live tasks (`lf pm show --wave <name> --no-sync`).
- Read the repo's agent doc (`CLAUDE.md` / `AGENTS.md`) for conventions.

Write design artifacts, notes, and open questions under `scratch/`. Don't
re-derive what these already record.

## Goal

Wave mitosis. The parent wave ceases to exist — its identity, projects, and tasks are distributed across N new waves.

The numeric argument controls how many children to create (default 2).

Linear tasks move as-is — each one lands in exactly one child. But `GOAL.md` needs rewriting, not slicing:

- **Intent**: written fresh for each child. Must be internally coherent, not a fragment of the parent's.
- **Metrics**: move each reviewed contract to exactly one child with its owning
  Project. Cross-Wave consumption is explicit routing, never a duplicated
  metric identity or observation stream.
- **Memory**: carry forward the decisions in `MEMORY.md` that each child still needs.

## Workflow

1. Read the parent wave
   - Use the wave passed by argument, or ask which `wave/<name>/` to split
   - Read `GOAL.md`, `MEMORY.md`, and the PM snapshot (`lf pm show --wave <parent> --json --no-sync`)

2. Find split boundaries
   - Look for thematic clusters, dependency chains, or independent workstreams
   - Aim for the requested count (default 2)
   - Each resulting wave should stand alone

3. Allocate tasks
   - Assign each Linear issue to exactly one child — no orphans

4. Create the new waves
   - `wave/<child>/GOAL.md` — fresh intent and process judgment for each child; draw scope boundaries between siblings
   - `wave/<child>/MEMORY.md` — the decisions and context this child inherits
   - `wave/<child>/metrics/*.md` — contracts whose owning Projects move to that child
   - `lf pm init --wave <child>` — create each child's Linear Initiative
   - recreate its allocated measured bets with `lf pm project create`
   - Move allocated tasks with `lf pm task move --id <task> --wave <child> --project <project>`. Archive the parent Projects after every task moved. Use `lf pm task done --id <task>` only for shipped work.

5. Remove the parent
   - Delete `wave/<parent>/`
   - Commit: `split-wave: <parent> → <child-a>, <child-b>`

6. Verify
   - Each child has a `GOAL.md`, a `MEMORY.md`, and a connected Linear Initiative
   - Every metric contract resolves to exactly one Project under exactly one child
   - No content from the parent is unaccounted for

## Guardrails

- Carry forward the original wave's intent — don't reshape the product direction during a split
- Concrete, domain-specific names for children
- When a boundary is unclear, pick the simpler option and note the alternative in `scratch/questions.md`
