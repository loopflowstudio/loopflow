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
  PM snapshot, and live tasks (`lf wave status <name>`).
- Read the repo's agent doc (`CLAUDE.md` / `AGENTS.md`) for conventions.

Write design artifacts, notes, and open questions under `scratch/`. Don't
re-derive what these already record.

## Goal

Wave mitosis. The parent's durable responsibility is distributed across N new
Waves. Keep its existing Linear Projects and dated evidence; historical plans
remain in Linear, with missing historical membership or measurements explicit.

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
   - Read `GOAL.md`, `MEMORY.md`, and the PM snapshot (`lf wave status <parent> --json`)

2. Find split boundaries
   - Look for thematic clusters, dependency chains, or independent workstreams
   - Aim for the requested count (default 2)
   - Each resulting wave should stand alone

3. Allocate tasks
   - Assign proposed new Tasks to one child. Account for every existing issue;
     active Tasks retain the parent Wave and delivery identity until settled.

4. Create the new waves
   - `wave/<child>/GOAL.md` — fresh intent and process judgment for each child; draw scope boundaries between siblings
   - `wave/<child>/MEMORY.md` — the decisions and context this child inherits
   - `wave/<child>/metrics/*.md` — contracts allocated to that child
   - `lf wave connect --wave <child>` — create each child's Linear Initiative
   - Connection does not create a Project. Inspect each child's Linear plans:
     one In Progress Project is current; Planned and Completed Projects retain
     future and historical content. Preserve existing Project identities.
   - If a current Project exists, `lf wave update-plan --wave <child> --plan <plan.json>`
     replaces its complete content: `{"metric_targets":[],"flow":"feature","krs":[]}`.
     Supply the intended nonempty `flow` string and all KRs and targets.
   - If no current Project exists, arrange its plan through the accepted
     repository chapter process or an explicitly authorized Linear change.
     Future plan authoring needs an available Linear writer; `update-plan`
     cannot write a Planned Project. Report missing access rather than inventing
     a command or starting Tasks without a current plan.
   - File new Tasks with `lf task create --wave <child> --title "…" --notes "…"`. Describe the problem, desired outcome, observable acceptance, and real constraints. Put allocation history in authorized comments; keep current blockers and dependencies in the description even when comments are collapsed.
   - Record existing active Task dependencies in the child plans; do not reparent a live delivery across Waves or restart its worker.

5. Retire the parent's future planning
   - A split does not authorize a chapter rotation. If a repository boundary is
     also accepted, preview `lf repo new-chapter <name> --dry-run --json` for all
     Waves, including the parent and children. There is no per-Wave or plan-file
     option. Apply with `lf repo new-chapter <name> --json` only after the
     direction and Task-disposition review gates; retain prior authorization.
   - Rotation moves started unfinished Tasks within their owning Wave, retaining
     worktrees, PRs and captured execution. Proven untouched backlog is canceled,
     retaining issues and history; terminal Tasks stay historical. Missing
     evidence stays unresolved. Rotation does not transfer Tasks to child Waves.
   - Predecessor Projects become Completed; an authored Planned successor keeps
     its content, otherwise only the predecessor's Flow is copied. New Waves
     without Projects receive an empty `feature` plan. Retry interruption with
     the same name. Keep the parent available while its active work finishes;
     do not delete its Projects or imply rotation retired the Wave itself.

6. Verify
   - Each child has a `GOAL.md`, a `MEMORY.md`, and a connected Linear Initiative
   - Every metric contract belongs to exactly one child Wave
   - No content from the parent is unaccounted for

## Guardrails

- Carry forward the original wave's intent — don't reshape the product direction during a split
- Concrete, domain-specific names for children
- When a boundary is unclear, pick the simpler option and note the alternative in `scratch/questions.md`
