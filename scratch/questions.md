# LOO-968 open choices

Choices made headless, each with how to reverse it. Names are today's
(`auto`, `lf task auto off`); the first build called them Auto-Ship and
`lf task hold`.

## What people said

Jack, 2026-11-19, on the concept review's "one idea, three words" (shipping
state, Autonomous section, Auto-Ship mark): "I tink this should be in scope"

Jack, 2026-11-19, on `scratch/explore.html`, quoting vision C ("A mark, not a
state: delete TaskState::Shipping and TaskSection::Autonomous. The Task reads
moving with an Auto-Ship badge…"): "this is cloest"

- "1. a mark/badge is a state isnt it?"
- "2) do orchestrator or humanlayer have terms they used here"
- "3) i think lf task auto on lf task auto off maybe?"
- "4) b"

Jack, on whether Auto-Ship becomes "auto" in product copy too: "ok lets take
the next pass. Just become an Auto toggle works."

Questions 1 and 2 got no written answer in scratch.

## Made for review

- **Off doesn't kill.** `lf task auto off` lets the run carry on to its next
  interactive step or landing. Reverse if a stop must be immediate.
- **Failures don't touch auto.** A run that fails the same way stops; auto
  stays on. Before, a terminal stop withdrew the approval.
- **No route approval.** A run with no interactive step ahead merges without
  writing auto on. Reverse by restoring the write in the run's start path.
- **One mark, not an on/off pair.** Who turned it on before an off is not
  kept. Reverse if someone needs it.
- **Hand-merged PRs go through the Graph.** No PR watcher; a graph with no
  land step finishes its steps into `end`.
- **Stored value `stopped`, shown as "off".** Reverse by renaming the value in
  the unmerged migration.
- **`auto on` on a Task with no Graph is an error naming `lf ship`** (was:
  take up the default graph, which made the Task read "Starting" and its
  conversation start a run). `auto off` there saves nothing and answers "auto
  was not on". Reverse by storing auto on the Task instead of `task_graphs`,
  so it can be set before the first run.
- **Reopening a Task clears auto.**
- **State never reads the setting**, so auto on, waiting at an interactive
  step with nothing running, reads `active` (was `idle`).
- **Rows learn auto through the selection state `auto`**, not a field on each
  row DTO. A done or abandoned Task never carries it.
- **Desktop's toggle on sends `task.ship`** (auto on, then the run). Reverse to
  `task.auto on` if the toggle should start nothing.
- **`lf task auto on` then bare `lf task run` continues after a waiting
  interactive step**; only `lf ship` or `--from <step>` stands in there.
- **A moved Task continues as `lf task run`** on the destination;
  `mini_starts_nothing_for_a_task_that_moved_back` counts mini's Graph runs.
- **A released `lf ship` that dies across the upgrade is not carried again.**
  Reverse by copying a live ship carrier's pid onto `task_graphs.run_pid` in
  the migration.
- `TaskGraphRecord.auto` carries `#[serde(default)]` and the Move JSON keeps
  its `approval` key for released peers; LOO-971 removes both.

## Open

- Whether `lf -b task run` should write auto on.
- A Graph process killed with its carrier stays open until the hourly prune,
  so the Task reads `moving` with nothing running. Base behavior;
  `lf machine prune` settles it.
- Whether `lf ship` should set auto inside the run's own take-up
  (`start_task_run`) instead of taking the Graph up just before it.
