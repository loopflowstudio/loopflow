---
requires: .lf/wave/<wave>/GOAL.md
produces: .lf/wave/<wave>/<child>/ directories
---
Split a wave into smaller, independent waves. The children take over future
work; the parent keeps its history and finishes its active Tasks.

Edit `.lf/wave/` only when the running Graph says to; otherwise put proposed Wave
changes in `scratch/`. Read `scratch/` first.

The numeric argument sets how many children to create (default 2). The parent's
Linear Projects and dated history stay; past plans remain in Linear, and any
missing past membership or measurements are noted as missing.

Each child gets a fresh GOAL, not a slice of the parent's prose:

- **Intent**: written new for each child and coherent on its own.
- **Metrics**: each metric moves to exactly one child. Another Wave that needs it
  reads it from there; never duplicate a metric or its readings.
- **Memory**: carry forward only the `MEMORY.md` entries a child still needs —
  direction, exact quotes, past observations, outside facts.
  Anything recoverable from the code or git history stays behind.

## Workflow

1. **Read the parent.** Use the wave from the argument, or ask which
   `.lf/wave/<name>/` to split. Read `GOAL.md`, `MEMORY.md` and
   `lf wave status <parent> --json`.

2. **Find the split.** Look for thematic clusters, dependency chains or
   independent workstreams. Hit the requested count; each child must stand
   alone.

3. **Allocate Tasks.** Assign each proposed new Task to one child. Account for
   every existing issue; active Tasks stay in the parent Wave until they finish.

4. **Create the children.**
   - `lf wave create <wave>/<child>` registers each child; commit its
     `.lf/wave/<wave>/<child>/` files on the main checkout and edit them there.
   - `GOAL.md`: fresh intent and judgment, with clear scope lines between
     siblings. `MEMORY.md`: what this child inherits. `metrics/*.md`: the
     metrics allocated to it.
   - `lf wave planning connect <wave>/<child>` links each child's Linear Initiative. Create
     starts the first Project; check each child's plan after connecting and keep
     existing Project IDs and Planned/Completed history.
   - `lf update-plan --wave <wave>/<child> --plan <plan.json>` replaces the
     current Project's whole content: `{"metric_targets":[],"krs":[]}`. Supply
     every KR and target; keep the Wave's default graph. `update-plan` can't
     write a Planned Project; future plans need a Linear writer.
   - File new Tasks with
     `lf task create --wave <wave>/<child> --title "…" --notes "…"`: the problem,
     the outcome wanted, how to tell it's done, and real constraints. Allocation
     history goes in comments; current blockers and dependencies stay in the
     description.
   - Note active Tasks' dependencies in the child plans. Never move a live Task
     across Waves or restart its worker.

5. **Retire the parent's future planning.**
   - Writing a plan is separate from splitting: each Wave's plan is written in
     place with `lf wave update-plan` after the direction is reviewed; see
     start-chapter. Nothing moves Tasks between Waves on its own.
   - Move a Task to a child only with `lf task move <issue> --wave <wave>/<child>`,
     and only unstarted work: a started Task keeps its worktree, PR, Sessions and
     Graph where they are. Keep the parent available while its active work
     finishes; don't delete its Projects.

6. **Check.**
   - Each child has `GOAL.md`, `MEMORY.md` and a connected Linear Initiative.
   - Every metric belongs to exactly one child.
   - Nothing from the parent is unaccounted for.

## Rules

- Keep the parent's intent; don't change product direction during a split.
- Give children concrete, domain-specific names.
- When a split line is unclear, pick the simpler option and note the other in
  `scratch/questions.md`.
