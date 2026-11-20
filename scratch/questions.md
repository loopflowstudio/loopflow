# LOO-968 open choices

Choices made headless, each with how to reverse it. Names are today's
(`auto`, `lf task auto on|off`); earlier builds called them Auto-Ship,
`lf ship` and `lf task hold`, and entries below that say "ship" mean
`lf task auto on`.

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
- **Stored value `off`** (was `stopped`). One name for what people
  type and what `--json` says.
- **`lf ship` is a run that asks for auto** (concept review, second pass).
  The run's take-up writes auto; `ship_on` is gone. A ship that fails
  before its run starts leaves auto unset. Reverse by restoring `ship_on`.
- **A ship on a done or abandoned Task gets `lf task run`'s sentence**, not
  "is complete" / "recover it before turning auto on".
- **A run that loses the start to another reads the Task again**; the bare
  run then says "is running graph …". The "moved on graph … while this run
  was starting" sentence is gone.
- **A ship waits up to 30 s for a just-started run to register its Graph**
  before reporting it; past that it gets the bare run's refusal.
- **A Task waiting at `human-review` keeps that name** in its captured Graph;
  no stored Graph is rewritten.
- **The run-outlook steer is in the plan as this Task's next section**, not
  filed: `DiedRun::reason` is code this PR adds. Reverse by filing it.
- **`lf task auto on` runs the Task** (concept review, 2026-11-19),
  standing in at a waiting interactive step. Reverse by restoring a mark-only write; to arm
  ahead of a first run instead, store auto on the Task.
- **`lf task auto on` with no ISSUE on a branch with no Task makes one**, as
  `lf ship` did.
- **The served off is `task.auto.off`**, answering `{issue, was_on}`.
- **`ships` became `merges`** on the run receipt.
- **Reopening a Task clears auto.**
- **State never reads the setting**, so auto on, waiting at an interactive
  step with nothing running, reads `active` (was `idle`).
- **Rows learn auto through the selection state `auto`**, not a field on each
  row DTO. A done or abandoned Task never carries it.
- **Desktop's toggle on sends `task.auto.on`** (auto on, then the run).
- **A moved Task continues as `lf task run`** on the destination;
  `mini_starts_nothing_for_a_task_that_moved_back` counts mini's Graph runs.
- **A released `lf ship` that dies across the upgrade is not carried again.**
  Reverse by copying a live ship carrier's pid onto `task_graphs.run_pid` in
  the migration.
- `TaskGraphRecord.auto` carries `#[serde(default)]` and the Move JSON keeps
  its `approval` key for released peers; LOO-971 removes both.

- **A died run the server will not start reads `blocked`** (no server, not
  claimed or checked out here), with the command; it read `waiting` before.
- **A run's death has a clock time only once retention records it**; until
  then the sentence is "Run died at <step> · …". Reverse by stamping
  `run_died_at` when a reader first finds the pid gone.
- **Runs and conversations share one start limit** (`CONVERSATION_STARTS`);
  `RUN_RESTARTS` is gone.

- **`ship_control` is deleted; the Auto toggle reads `run_control`** (concept
  review, third pass). A Task at `end` whose completion is pending shows an
  enabled toggle that answers `ended`. Reverse by restoring the field as
  `auto_control` with the at-`end` reason.
- **`lf ship --reason` is the run's direction and `lf task auto off --reason`
  is a Task comment** (same pass); the mark's write takes no note. Reverse by
  a note column on the mark that `lf task status` prints, or by deleting both
  flags.
- **The off reason is written as a person's comment, not a steer, and is
  published to the tracker.** From a terminal `append_task_comment` gives it
  the same body shape as a steer. Reverse by passing `steer: true` or by
  using the store-only write.

- **Auto is stored on the Task, in its own `task_auto` row** (concept review,
  sixth pass, 2026-11-19), so `lf task auto off` before a first run is kept
  and outlasts a config `auto: on`, and a released hold on a never-run Task
  survives the upgrade. Passed over: columns on `tasks` (a planning table
  whose update triggers compare every column), and a personal `auto: off`
  config value. Reverse by putting the three columns back on `task_graphs`.

## Open
- QA 2026-11-19 (fourth pass): `WaveSetup` gained `auto` so `lf wave status`
  prints it. Reverse by dropping the field and the README's pointer.
- QA 2026-11-19 (third pass): a Task conversation's input carries no Graph
  position at all; the deleted `TaskGraph::guidance` was its only writer and
  reached no provider. Reverse by appending it to `task_input::task_seed`
  (read once at a conversation's start, so it goes stale as the run moves).
- QA 2026-11-19 (third pass): a hand-merged Task waiting at land with auto
  off reads `latest Graph: Waiting at land … lf ship X lands it` beside
  `action: no_action (Merged; complete the Task)`. Both are true; `lf task
  auto on` takes it to `done`.
- QA 2026-11-19 (second pass): `lf task auto off --reason` reaches the next
  step's agent as a steer ("I want to read the diff first" was pending for the
  ship-review Session that a later ship started). Same root as the recorded
  comment-shape choice; reverse with the store-only write.
- QA 2026-11-19: a served `task.ship` on an unknown issue answers
  `outcome_unknown` ("…; inspect the affected work before retrying"), as
  `task.run` does through the shared launch path. `task.auto.off` now answers
  `not_found`.
- QA 2026-11-19: a released hold on a never-run Task does not survive the
  upgrade; same root as the last item below.
- QA 2026-11-19: Task checkout waits 10 s for the worktree lease instead of
  failing at once. Reverse by restoring `acquire_worktree_lease` in
  `restore_task_checkout`; two starts at once then fail one caller.

- A Graph process killed with its carrier stays open until the hourly prune,
  so the Task reads `moving` with nothing running. Base behavior;
  `lf machine prune` settles it.

## Ship review (2026-11-19)

Jack: "lets also rename human-review to ship-review. And we can discuss quickly whether we ant to /refine or change its goals so it makes sense both interactive and not. I would say that at interactive its often my goal to basically:
(a) try to minimize comprhension debt, make sure i actually understand what is being shipped
(b) make some calls on how aggressively to go after things found during concept review [often in particular the automated concept reivew says "outside this PR" -- i may disagree or just want to explicitly start those tasks
mostly i think we dont need like a true "code review" or go/no decisions -- concept review + gate covers that. so an automated ship review is probably mostly about filling followups for anything not covered in the concept-review, or potentially fixing some small things,"
- Jack, on headless filing: "(a) file every untracked finding as a Task" / "Generally perfer bulk tasks for a bunch of small things rather than just explosive task fan out"
- Jack: "also encourage the ship review to only follow things that are clearly important, but just too big and too scope-expansive to fix here. We should make sure concept-review is encouraged to be aggressive about being willing to take another big pass to get the models right. / or small stuff, generally either fix it here or just dont file it, unless theres a really good reason to separete it."

## Run outlook (2026-11-19)

Jack, as a comment on this Task: "From LOO-959's concept review (answer 1a, deferred to this Task because 959 merged first as #1972): give the Task run the conversation's outlook type. The run's died {at_step, at} and the SessionOutlook shape replace DiedRun::reason in waiting/blocked; 'no server runs here' applies to the run too; the three run sentences use the conversation's words (clock time, 'starting again', no 'pass'). Fold it into this Task if it fits after ship's restart rule; otherwise file it as its own Task."

## The auto graph (2026-11-19)

Jack, quoting option 3 ("Delete it. … unless verify also stands in when auto is on."): "Should we allow a fully auto setting instead?"
Then: "so that waves can have default graphs with an optional auto modifier"
Open: `-b` — offered "batch only, writes no auto" (1), keep (2), `--auto` (3); no answer yet. Then a steer (Jack Heart): "-b keeps standing in at every interactive step for its own run and the caller waits, as on any Graph; it writes no auto. Do not remove the launcher.batch stand-in in stands_in; only remove the auto write." Built that way.
- Made for review: the config default applies at every take-up (also `lf task run X <other graph>`), not only the first, and only when auto is unset. Attribution is a new `Starter::Config` variant rather than a separate type for auto's setter.
- Jack, on `-b`: "no i think its right as it is -- -b should work on task run much like it does a regular graph". Plan: `-b` stands in for its own run, writes no auto.
- Jack: "lets kill lf ship; isnt that just lf task auto on?"
