---
requires: .lf/wave/<wave>/GOAL.md
produces: .lf/wave/<wave>/<child>/ directories
---
Split a Wave into smaller, independent Waves. The children take over future work; the parent keeps its history and finishes its active Tasks.

The numeric argument sets how many children to create (default 2). Keep the
parent's intent; a split doesn't change product direction.

Each child gets a fresh goal, not a slice of the parent's prose:

- **Intent**: written new for each child and coherent on its own, with clear
  scope lines between siblings and a concrete, domain-specific name.
- **Metrics**: each metric moves to exactly one child. Another Wave that needs
  it reads it from there; never duplicate a metric or its readings.
- **Memory**: carry forward only what a child still needs: direction, exact
  quotes, past observations, outside facts.

## Method

1. **Read the parent.** `GOAL.md`, `MEMORY.md` and
   `lf wave status <parent> --json`. With no Wave named, ask which.

2. **Find the split.** Thematic clusters, dependency chains or independent
   workstreams. Hit the requested count; each child must stand alone. When a
   line is unclear, pick the simpler option and note the other in
   `scratch/questions.md`.

3. **Account for every Task.** Assign each proposed new Task to one child.
   Active Tasks stay in the parent until they finish.

4. **Create the children.**
   ```bash
   lf wave create <wave>/<child>                    # registers it and commits its files on main
   lf wave plan connect <wave>/<child>              # links its Linear Initiative
   lf wave update-project --wave <wave>/<child> --file <project.json>   # {"metric_targets":[],"krs":[]}
   lf task create --wave <wave>/<child> --title "…" --notes "…"
   ```
   Write each child's `GOAL.md`, `MEMORY.md` and `metrics/*.md` in its
   directory. `update-project` replaces the current Project's KRs and targets, so
   supply every KR and target.

5. **Move only unstarted work,** with
   `lf task move <issue> --wave <wave>/<child>`. A started Task keeps its
   worktree, PR, Sessions and Graph where they are; never move it or restart
   its worker. Keep the parent and its Projects while its active work
   finishes.

6. **Check.** Each child has a goal, memory and a connected Initiative. Every
   metric belongs to exactly one child. Nothing from the parent is
   unaccounted for.
