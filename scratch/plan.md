# LOO-968: Ship is shorthand for auto on the Task's Graph

> "i feel like ship is supposed to be a small short hand that maybe updates the
> proposed graph to be auto mode, and has grown into this sprawling concept.
> lets reduce its impact on the overall system and protocol" — Jack, 2026-11-18

Later direction, word for word, is in `scratch/questions.md`: the shipping
state and Autonomous section in scope, vision C ("a setting and no state"),
`lf task auto on|off`, the Auto toggle, ship-review, the run outlook, the
config `auto: on` modifier, `-b`, and "lets kill lf ship; isnt that just lf
task auto on?".

PR #1976, base `main`. Main (through #1985) is merged in at `726f8a320f`.

## What exists

Status and setting are separate axes. `TaskState` says what is happening; the
auto setting says who takes the interactive steps. One word: **auto**.

- **The setting** is one `task_auto` row per Task: `state` (`on` | `off`),
  `set_at`, `set_by` (`Starter` JSON resolved at write time); no row when
  never set. Rust `Option<Auto>`, `AutoState { On, Off }`. A Task holds it
  before its first run and through every Graph it takes up, it moves with the
  Task, and reopening clears it. Migration `task_auto` adds the table and
  `task_summaries.auto`, backfills from `task_ship_events`, NULLs
  `tasks.shipped_at` (so a released lf during upgrade skew stops relaunching
  `lf ship`), and marks `task_ship_events`, `tasks.shipped_at` and
  `task_summaries.ship_events` leaving for LOO-971.
- **Who sets it**, and nothing else does:
  - `lf task auto on [ISSUE] [--graph <g>] [--reason] [--json]`, served as
    `task.auto.on`. It is the run, asked for auto: detached, `lf task run`'s
    output, answering `TaskRun`. On a live run it turns auto on and reports
    that run (`merges: true`); on a Task waiting at an interactive step it
    stands in there; at `end` it answers `Ended` and writes nothing. With no
    ISSUE it takes the checkout's Task, or makes one for the branch. It is the
    only thing that lifts an off.
  - `lf task auto off [ISSUE] [--reason] [--json]`, served as `task.auto.off`,
    answering `{issue, was_on}`. It kills nothing: the run stops at its next
    interactive step or at landing. `--reason` is a Task comment.
  - `auto: on` in a Wave, repository or personal config, written as a Task
    takes up a Graph and only when its auto is unset, attributed to the layer
    (`Starter::Config`, "auto: on by wave config").
- **`lf ship` is gone**, with `Commands::Ship`, `task.ship`, `lf task hold`,
  `task.hold`, the ship carrier loop and its log API, `ShippingSnapshot`,
  `TaskShip`, `ship_control` and the builtin `auto` graph. `lf ship` now reads
  "skill or graph not found: ship". A released config's `graph: auto` reads as
  `graph: code` + `auto: on` (`keys::fold`) until Task d3af77 removes it.
- **`-b` on `task run`** works as on any Graph: that run stands in at every
  interactive step and the caller waits. It writes no auto, so a died `-b` run
  restarts as a bare run and stops at its next interactive step.
- **What reads the setting:** the Graph runner at each interactive step of a
  Task run (`RunningGraph::stands_in`), and the landing step, which waits only
  on `off` (`TaskGraph::waiting_to_land`: "Waiting at land: auto is off
  (Maya). `lf task auto on X` lands it."). `task_state_sql` and
  `TaskSection::of` never read it.
- **No `shipping` state, no Autonomous section.** A Task with auto on reads
  `moving` in Working while it runs. Rows learn the setting through the
  selection state `auto` (`state:auto`).
- **One recovery path:** a dead run of any Task is carried again as
  `lf task run ISSUE` under the conversation's start limit
  (`CONVERSATION_STARTS`, 5); a moved Task continues the same way.
  `TaskExecutionSnapshot` carries `died {at_step, at}` and `outlook`; `reason`
  is the one sentence, written in Rust.
- **Surfaces:** `lf task status` first line `LOO-1  moving · auto: on by Maya,
  Nov 18 09:04`; snapshot field `auto`; Desktop `TaskAutoControl` toggle (on
  sends `task.auto.on`, off `task.auto.off`, enabled by `run_control`) and
  `AutoBadge`; docs, builtin skills, `.lf/qa/auto.md` and AGENTS.md say auto.
- **`human-review` is `ship-review`**, step and skill. A Task waiting at
  `human-review` in a stored Graph keeps that name until it takes a Graph up
  again.

`refs/wip/loo-968-auto-ship-rename` holds a dropped rename-only diff; delete it
when the PR merges.

## Remaining

Gate. Nothing is left to build.

Implement 2026-11-19, the concept review's slice (auto is the Task's, so it is
stored on the Task):

- `task_auto(task_id, state, set_at, set_by)`, one row per Task that has it
  set; the migration is renamed `task_auto.sql` and backfills every Task's
  latest ship event, with or without a Graph. `set_at`/`set_by` because `by`
  is an SQL keyword. Its three `store_revision_task_auto_*` triggers advance
  `planning`.
- `set_task_auto` writes the row for any Task (`INSERT OR IGNORE` when
  `unset_only`); `take_up_task_graph` copies nothing; reopening deletes the
  row; `task_begun` reads no auto. No fixture needed the dropped clause.
- The mark travels as `MovedTask.auto` and is placed whether or not a Graph
  came with it (`place_moved_task_auto`: the sender's mark or none replaces
  what the receiver held, with no history line). A released sender omits it
  and its Task arrives with none, as before.
- `lf task auto off` prints "X: auto off" either way.
- A dev store that applied the old `task_graph_auto` draft has `auto*`
  columns on `task_graphs` and no `task_auto`; QA worlds made before this
  commit are rebuilt, not upgraded.

Run here: storage `an_auto_off_stands_until_auto_on`,
`a_moved_task_keeps_its_auto_mark_where_it_arrives`,
`a_released_tasks_ship_log_becomes_its_auto_mark` (its stopped Task has no
Graph row), `auto_reads_and_writes_plan_by_index`, `revisions::`, the
migration runner tests (152 passed); application
`an_off_before_the_first_run_outlasts_a_waves_auto_on`, the four neighbouring
auto launch fixtures, `end_is_retained_while_the_tasks_pr_blocks_completion`,
`engine::task::`, `dto_fixtures::`, `server::schema::tests::` (schema
regenerated: two doc comments), `served_reads`'
`server_operations_match_their_lf_commands` (it asserted the lost off; now
asserts the off is kept), `documented_commands::`; clippy clean. Not run:
`two_people`, `transport::`, `session_lifecycle_tests`, `performance_tests::`
and the Swift suites the testing page names for a schema change; gate and CI.

Realign at `9c657bfb39` read the slice against its brief: no `task_graphs`
auto reference is left, the fixture and the upgrade test exist as named.
`docs/orchestration.md` still listed the deleted builtin `auto` graph and said
`pursue` lands; `pursue` ends at `pr-publish`. It now names `pursue` as
request to published PR, and the `auto: on` key.

Realign 2026-11-19 at `76b2bd214e` ran the two fixtures that still named the
deleted `auto` graph: `local_planning::auto_graph_runs_a_task_to_end` (waited
60 s at `code`'s first step) and
`…::your_graph_for_a_wave_selects_the_builtin_auto` ("graph not found: auto").
Both are deleted; `a_waves_auto_on_takes_every_step_of_its_tasks_and_an_off_still_wins`
and `keys::tests::a_released_auto_graph_reads_as_code_with_auto_on` hold what
they stood for. `wave_lifecycle`'s fold fixture names `graph: feature`
(passed). `docs/dev/index.html` regenerated (it still said `lf ship`).

Checks nobody has run since the last three commits (`1048cee3cf`,
`fa171df836`, `76b2bd214e`); gate and CI:

- `golden_prompt` (skill text changed; regenerate), `config::` catalogue
  tests. (`documented_commands::` and `dto_fixtures::` passed at `9c657bfb39`.)
- Swift: `scripts/test_desktop.sh --filter 'DTOFixtureTests|ServerProtocolTests|RegistryQueryTests|WorkModelTests|WorkNavigationTests|WaveStandupTests|WorkSelectionTests|TaskStatusTests|TaskConceptTests|DesktopHeadlessTests|PendingChangesTests|WorkCacheTests|TaskRowMetadataTests'`.
  Before main was merged in: 285 tests, 3 failures, all the base's
  `WorkPart(rawValue: "activity")`.
- iOS (`uv run python scripts/test_ios.py`): `TaskWorkspaceView.swift` and
  `TaskListView.swift` were edited and never compiled.
- `served_reads` beyond `server_operations_match_their_lf_commands`,
  `machine_commands` (both edited for the auto operations),
  `performance_tests::`, `world_recipes::`, `two_people`, the rest of
  `local_planning`, the website docs check.
- Recovery fixtures that time out here with and without this branch's edits
  ("waiting for the restart", "waiting for the Task conversation and Graph
  launches"): `a_restarted_wait_keeps_its_explicit_agent`,
  `an_auto_tasks_restarted_wait_keeps_its_explicit_agent`,
  `a_restarted_wait_with_no_explicit_agent_ignores_the_readers`,
  `a_task_run_takes_its_waves_agent_without_flags`,
  `a_task_run_whose_carrier_died_is_carried_again_and_gives_up_after_five`.
  Gate reads what CI says; a test that fails there is fixed or deleted in this
  change.
- `an_auto_run_meets_a_hand_merged_pr_at_its_landing_step` and
  `a_run_that_fails_the_same_way_stops_and_keeps_auto_on` failed once on
  "read the fixture's Machine server status" beside a Swift build and passed
  at `-j 2`.
- Not driven by anyone: the Desktop window (toggle and badge), iOS, an auto on
  arriving in the 30 s before a carrier's Graph registers, a moved Task, and
  the last three commits as a person would use them (config `auto: on`,
  `lf task auto on --graph`).

Last run here, 2026-11-19 after `fa171df836`: `cargo nextest run -p loopflow
-p loopflow-execution --lib` over `lf::tests::`, `adapter_tests::`,
`server::schema::tests::`, `lf::commands::write::tests::`,
`config::keys::tests::`, `engine::task::` and the auto/batch/Wave-auto
`task_graph_launch_tests`: 85 run, 3 failed on stale `lf ship` assertions,
fixed in `76b2bd214e`, the 9 rerun passed. Clippy clean.

Compress 2026-11-19: `on` and `off` are Clap subcommands of `lf task auto`
(`AutoCommand`), so `off --graph` is a parse error and each has its own help.
`ServerProtocolTests` still named the deleted `task_ship` and
`task_ship_answer` fixtures; fixed, 23 tests pass
(`scripts/test_desktop.sh --filter ServerProtocolTests`).
`a_bare_ship_stands_in_because_auto_is_on` repeated
`auto_on_at_a_waiting_step_stands_in_there_with_one_graph_run` and is deleted;
the other launch fixtures say auto, not ship. Run here: `lf::tests::`,
`adapter_tests::`, `server::schema::tests::`, `write::tests::` (59),
`documented_commands`, `cli_discovery`, the auto launch fixtures; clippy clean.

Compress 2026-11-19, second pass: nothing structural was left to cut. Gone:
the last readers of `task_ship_events` (`query_plans`' growing-table list and
measured stats, which now name `task_auto`, so
`auto_reads_and_writes_plan_by_index` checks the table it writes; the pid
queries in test support and `task_create_tests`), and the duplicate mark
`INSERT` (`write_auto_in`).

QA 2026-11-19, fourth pass at `2c53f20ade` (`scratch/qa.html`): `lf ship` and
`lf task hold` name nothing; `auto: on` from your own, the repository's and a
Wave's config turns auto on at take-up and status names the layer; an off
outlasts the config, another graph and a person's run; `auto on --graph
--reason`; `-b` writes no auto and its killed run came back as a plain run; a
released `graph: auto` reads as `code` + `auto: on`; lf 0.16.0 beside a config
with `auto: on` warns "skipping auto: unknown setting" on every read and
carries on. Fixed: `lf wave status` now lists `auto` in a Wave's setup
(`WaveSetup.auto`, schema and `wave_detail.json`). Run: 60 Rust tests over
`each_wave_runs_its_own_setup`, `dto_fixtures::`, `server::schema::tests::`;
Swift `DesktopHeadlessTests|ServerProtocolTests|DTOFixtureTests|TaskStatusTests|TaskRowMetadataTests`,
107 passed; clippy clean. The Dev app launched on the world and its socket
read the Task page; the screen was locked, so the toggle and badge are still
unseen. iOS not compiled.

Shape, for gate's review and ship-review's filing:

- Off means two things: the person takes the interactive steps, and nothing
  merges. After `auto off`, a person's own `lf task run` past review waits at
  land until `lf task auto on`. Never set differs: such a run lands. Kept: an
  off said while a stood-in run is past review must still stop the merge.
- `TaskExecutionSnapshot.outlook` is on the wire and decoded in Swift, and no
  surface reads it: CLI and Desktop print `reason`.
- `auto::running_graph` polls up to 30 s for a carrier's Graph to register so
  `TaskRunStart::Live` can name a Graph run. If `Live` named the carrier, the
  wait would go.
- A Task waiting at an interactive step reads "latest Graph: Graph … finished;
  nothing further is launched" (base behavior).
- `task.shipped` meaning "merged".

## What the work taught

- A bare `lf task run` on a Task waiting at an interactive step means "the
  person finished this step" and continues after it, whatever auto says. That
  is why auto on is the run and not a mark followed by a bare run.
- An earlier build let `lf task auto on` take up a Graph without running it.
  The Task read "Conversation: starting", "latest Graph: Starting feature":
  a Task that took a Graph up and never ran it owes setup
  (`ops/human_session/setup.rs`), its conversation started `lf task run`
  detached, and the person's own run then met that live run and refused.
- State never reads the setting, so auto on with nothing running at an
  interactive step reads `active`, not `idle`.
- With the take-up inside the run, two first runs at once both took a Graph
  up and the second's `INSERT OR REPLACE` wiped the first's run. A first
  take-up is `take_up_first_task_graph` (`INSERT OR IGNORE`); the loser reads
  the Task again.
- The Task conversation's `TaskGraph::guidance` reached no provider (a Task
  launch's message comes from `task_input::task_seed`), so it was deleted.
- A released `lf ship` that dies across the upgrade is not carried again; the
  Task reads `idle`, auto on, and `lf task run` carries it. Reverse by copying
  a live ship carrier's pid onto `task_graphs.run_pid` in the migration.
- Hand-merged PRs: the land step meets the merge. Graphs with no land step
  finish into `end` and completion settles.

## What QA showed (2026-11-19, `scratch/qa.html`, when the command was `lf ship`)

On a private lf-world Machine with the branch `lf` and its server:

- A second auto on at the same moment exited 1 with "acquire worktree lease:
  … is owned by another active operation"; `restore_task_checkout` now waits
  up to 10 s (`CHECKOUT_LEASE_WAIT`). With three at once, 1 Task in 5 had one
  exit 1 with "the Task's run is carried by another `lf task run`";
  `try_start_task_run` now reads the Task again when the row names a live
  carrier. After both: 12 Tasks, three each, every exit 0, one Graph run each.
  Neither fix has a fixture; the race is not deterministic from outside.
- Working: auto on for a never-run, waiting, running, landing-wait, done and
  Task-less-branch start, with `--reason`; `auto off` mid-run stops at land; a
  failed land stops after 3 and keeps auto on; reopen clears auto; both
  operations over the socket and racing the CLI (one run, both exit 0);
  `state:auto` in a Dashboard query; from a hand-merged Task's checkout to
  `done`.
- A died run with no server read "Run died at implement · no server runs here
  to start it; `lf task run cf17c1` starts it again"; with a server, "… ·
  starting again", carried in about a minute.
- Upgrade from the installed 0.16.0 (`f5ff6a6c7`): a live released ship reads
  `moving · auto: on, Nov 19 10:51` and finished; a Task waiting at
  `human-review` went on with auto on (the captured Graph carries the old
  skill text).
- Fixed from QA: `lf task status` at a landing wait with auto off now reads
  the runner's sentence as `execution.state: waiting`, `action: no_action`;
  `lf task auto off` on an unknown ID answers `not_found`.

## Exclusions

- An `auto:` key in graph YAML: auto is a Task fact, not a graph property.
- The contract migration that drops what is leaving: LOO-971.
- Removing the `graph: auto` config fallback: Task d3af77.
