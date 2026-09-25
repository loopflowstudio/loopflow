---
requires: wave/<wave>/GOAL.md
produces: wave/<child>/ directories
---
Decompose a wave into smaller, independent waves. Move future responsibility into the children; retain the parent's history and any active delivery until it settles.

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

Wave mitosis. The parent's durable responsibility is distributed across N new Waves. Historical chapters remain inspectable.

The numeric argument controls how many children to create (default 2).

Assign new work to one child. Existing active Tasks keep their owning Wave and delivery identity until they settle. Rewrite each child’s GOAL rather than slicing prose:

- **Intent**: written fresh for each child. Must be internally coherent, not a fragment of the parent's.
- **Metrics**: move each reviewed contract to exactly one child with its owning
  Wave. Cross-Wave consumption is explicit routing, never a duplicated
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
   - `wave/<child>/metrics/*.md` — contracts allocated to that child
   - `lf pm init --wave <child>` — create each child's Linear Initiative
   - `lf wave update-plan --wave <child> --plan <plan.json>` — author its one chapter plan
   - File new Tasks with `lf pm task create --wave <child> --title "…"`.
   - Record existing active Task dependencies in the child plans; do not reparent a live delivery across Waves or restart its worker.

5. Retire the parent's future planning
   - Use `lf wave new-chapter --wave <parent> --chapter <id> --dry-run --json` to account for every active Task and untouched backlog.
   - Apply only the accepted preview. Keep the parent available while carried work finishes; never delete its history.

6. Verify
   - Each child has a `GOAL.md`, a `MEMORY.md`, and a connected Linear Initiative
   - Every metric contract belongs to exactly one child Wave
   - No content from the parent is unaccounted for

## Guardrails

- Carry forward the original wave's intent — don't reshape the product direction during a split
- Concrete, domain-specific names for children
- When a boundary is unclear, pick the simpler option and note the alternative in `scratch/questions.md`
