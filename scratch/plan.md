# LOO-968: Ship is shorthand for auto on the Task's Graph

> "i feel like ship is supposed to be a small short hand that maybe updates the
> proposed graph to be auto mode, and has grown into this sprawling concept.
> lets reduce its impact on the overall system and protocol" — Jack, 2026-11-18

After the first build, on the concept review's "one idea, three words"
(shipping state, Autonomous section, Auto-Ship mark): "I tink this should be in
scope". Then, choosing from `scratch/explore.html`: "this is cloest" (vision C,
a setting and no state); "i think lf task auto on lf task auto off maybe?";
"ok lets take the next pass. Just become an Auto toggle works."

PR #1976, base `main`. Main (with #1972 and #1974) is merged in at
`39b87d6262`.

## Auto on is one action

Concept review, 2026-11-19 (`scratch/concept-review.html`), a choice made for
review: auto on means nobody is waited for, so turning it on runs the Task.
`lf task auto on X`, `lf ship X` and Desktop's toggle are one operation.

Built 2026-11-19:

- `lf task auto on [ISSUE] [--reason] [--json]` parses as `lf ship`
  (`Cli::checked` in `rust/loopflow/src/lf/mod.rs`), so every dispatch, detach
  and remote route that names `Commands::Ship` serves both spellings. With no
  ISSUE on a branch with no Task it makes one, as `lf ship` does.
- `ops::task::ship::auto_on` and its no-Graph refusal are gone. On the wire
  `task.ship` is the one way to turn auto on; `task.auto` became
  `task.auto.off {directory, issue, reason}` answering `{issue, was_on}`
  (`AutoOff`). Fixtures `task_auto_off.json`, `task_auto_off_answer.json`.
- `ships` is `merges` on `TaskRun::Running` and `TaskRunStart::Run`, printed
  `merges:`.
- Run recovery lives in `ops/task/run_recovery.rs`.
- Tests: `auto_on_at_a_waiting_step_stands_in_there_with_one_graph_run`,
  `auto_on_a_task_that_never_ran_leaves_exactly_one_run`, and the parse row in
  `lf::tests`.

Run 2026-11-19 at `56e542adac`: `cargo test -p loopflow --lib --
task_graph_launch_tests::auto_o lf::tests`, 41 passed (the two `auto_on_*`
fixtures, `auto_off_sticks_through_a_headless_relaunch`, the parse tests).
Not run since this change: Swift `ServerProtocolTests`, `RegistryQueryTests`
and iOS (`RegistryQuery.swift`, `WorkModel.swift`, `TaskWorkspaceView.swift`
were edited for `task.auto.off` and `merges`). Gate and CI.

Passed over: keeping `auto on` a pure setting stored on the Task so it can
precede a Graph, with a bare run standing in at the waiting step. That keeps
two actions and changes what a bare `lf task run` means at an interactive step
("I finished this step"). Lost on purpose: arming auto on a waiting Task
without starting anything.

## What exists


Status and setting are separate axes. `TaskState` says what is happening; the
auto setting says who takes the interactive steps. One word: **auto**.

- **The setting** is one mark on `task_graphs`: `auto` (`on` | `off`, NULL
  when never set), `auto_at`, `auto_by` (`Starter` JSON resolved at write
  time). Rust `Option<Auto>`, `AutoState { On, Off }`. It moves with the
  Graph record, survives taking up another graph, and is cleared by reopening
  the Task. Migration `task_graph_auto` adds the columns and
  `task_summaries.auto`, backfills from `task_ship_events`, NULLs
  `tasks.shipped_at` (so a released lf during upgrade skew stops relaunching
  `lf ship`), and marks `task_ship_events`, `tasks.shipped_at` and
  `task_summaries.ship_events` leaving for LOO-971.
- **Who sets it:** `lf ship`, spelled `lf task auto on` too (on, always, the
  only thing that lifts a stop; served as `task.ship`); `lf task auto off
  [ISSUE] [--reason] [--json]` (served as `task.auto.off`);
  `lf -b task run` (on, unless off). Nothing else: no route
  approval, and failures, refusals and reaching `end` leave it alone.
- **`lf ship <task>`** is the run, asked for auto (detached, `lf task run`'s
  output, `task.ship` returns `TaskRun`). On a live run it reports that run. On
  a Task waiting at an interactive step it stands in at that step.
- **What reads it:** the Graph runner, at each interactive step of a Task run
  (`RunningGraph::stands_in`), and the landing step, which waits only on
  `off` ("Waiting at pr land: auto is off (Maya). `lf ship` lands it.").
  `task_state_sql` and `TaskSection::of` never read it. `auto off` kills
  nothing: the run stops at its next interactive step or landing.
- **No `shipping` state, no Autonomous section.** A Task with auto on reads
  `moving` in Working while it runs. Rows learn the setting through the
  selection state `auto` (`state:auto`).
- **One carrier, one recovery path:** a dead run of any Task is carried again
  as `lf task run ISSUE` under the conversation's start limit
  (`CONVERSATION_STARTS`, 5, then it waits for a person);
  a Task that moves Machines continues the same way. The ship carrier loop,
  `restart_ship`, `ship_halt`, `ship_settle_merge`, the ship log API,
  `ShippingSnapshot`, `TaskShip`, `lf task hold`, `task.hold` and the
  `ship --stop` alias are deleted.
- **Surfaces:** `lf task status` first line `LOO-1  moving · auto: on by Maya,
  Nov 18 09:04`; snapshot field `auto`; Desktop `TaskAutoControl` toggle (on
  sends `task.ship`, off `task.auto.off`) and `AutoBadge` on cards and rows; docs,
  builtin operate skills, `.lf/qa/auto.md` and AGENTS.md say auto.

`refs/wip/loo-968-auto-ship-rename` holds a dropped rename-only diff; delete it
when the PR merges.

## What the work taught

- A bare `lf task run` on a Task waiting at an interactive step means "the
  person finished this step" and continues after it, whatever auto says. That
  stays; it is why `lf task auto on` becomes the ship operation instead of a
  mark followed by a bare run.
- State never reads the setting, so auto on with nothing running at an
  interactive step reads `active`, not `idle`.
- A released `lf ship` that dies across the upgrade is not carried again (its
  carrier lived on `task_ship_events`); the Task reads `idle`, auto on, and
  `lf task run` carries it. Reverse by copying a live ship carrier's pid onto
  `task_graphs.run_pid` in the migration.
- Hand-merged PRs: the land step meets the merge
  (`a_ship_meets_a_hand_merged_pr_at_its_landing_step`). Graphs with no land
  step finish their steps into `end` and completion settles; no merge check
  between steps.

## Remaining

1. The last ship words on the wire (below).
2. Gate.

### Next: the last ship words on the wire

Concept review, third pass, 2026-11-19 at `cb20b422a6`
(`scratch/concept-review.html`). Ship is the run asked for auto, so what
makes it unavailable is what makes the run unavailable, and a reason goes
where reasons are read. Choices made for review; each is reversible.

Start with `ship_control`: it shows soonest whether Desktop holds with one
control.

1. **Delete `ship_control`.** It sits on every Task row beside `run_control`
   (`RoadmapTask` in `rust/loopflow/src/lf/commands/waves.rs`, built near
   line 2196), with one reader, the Auto toggle
   (`swift/LoopflowMac/Views/TaskWorkView.swift`, `toggle(refusal:)`). Its
   reasons: `run_control.unavailable`; "auto is already on", which the toggle
   never reads; "Task is at the end of its graph", which `lf ship` does not
   refuse (it answers `ended`). The toggle takes `task.runControl.unavailable`.
   Goes with it: `shipControl` in `swift/Loopflow/Models/WaveWorkMap.swift`,
   the key in `swift/LoopflowMac/WorkCache.swift` and
   `MockWaveFixture.swift`, the `steps` fact in `waves.rs` if
   nothing else reads them, the key in the 12 fixtures under
   `tests/fixtures/dto/` (`grep -rl ship_control tests/fixtures`), and
   `server_schema.json` regenerated. Kept on purpose: a Task at `end` whose
   completion is pending shows an enabled toggle; turning it on answers
   `ended` and leaves auto unset. Passed over: renaming it `auto_control`.
   Check: `dto_fixtures::`, `server::schema::tests::`, and
   `scripts/test_desktop.sh --filter 'DTOFixtureTests|WorkCacheTests|DesktopHeadlessTests|TaskStatusTests'`.
2. **`--reason` goes where reasons are read.** Today `set_task_auto` folds it
   into a `progress` Task event no read prints, under help that says "kept in
   the Task's history". `lf ship --reason` becomes the run's own
   (`TaskProcessOptions.reason`, the steer `task_place` already appends), in
   `bin/lf.rs`; `AutoAsk::Person` becomes a unit variant. `lf task auto off
   --reason` is saved as a Task comment (`append_task_comment`, not a steer)
   in `auto::off`. `set_task_auto` and `auto::set` lose `note`; the progress
   row says "auto on: Maya". Help for `lf ship`, `lf task auto` and the
   served `task.ship` / `task.auto.off` `reason` fields say what happens
   (`rust/loopflow/src/lf/mod.rs`, `server/wire.rs`). A test: after
   `lf task auto off X --reason why`, the Task's comments hold "why". Passed
   over: a note on the mark printed by `lf task status` (a column and a wire
   field), and deleting both flags.
3. **`docs/dev/reference-models.md`** still names "ship event" among the
   records that own a process (LfProcess row) and "`lf ship`" among the
   spawners. Both go.
4. **`task_shipping.json`** (the `task.ship` answer, under
   `rust/loopflow-execution/tests/fixtures/dto/server/`) becomes
   `task_ship_answer.json`; readers are `server/schema.rs` and
   `lf/commands/write.rs`.

Larger than this work, on the page and not filed: the "Graph … finished;
nothing further is launched" sentence on a Task waiting at an interactive
step (base behavior); auto stored on the Task so it can precede a Graph; the
30 s wait in `auto::running_graph`; `task.shipped` meaning "merged"; the
`auto` Graph's name. Ship-review decides which become Tasks.

QA 2026-11-19 at `75abe5ce1c` plus two fixes (`scratch/qa.html`), on a private
lf-world Machine with the branch `lf` and its server:

- Fixed: a second ship at the same moment exited 1 with "acquire worktree
  lease: … is owned by another active operation". `restore_task_checkout` now
  waits up to 10 s for the checkout lease (`CHECKOUT_LEASE_WAIT`).
- Fixed: with three ships at once, 1 Task in 5 had one ship exit 1 with "the
  Task's run is carried by another `lf task run`". `try_start_task_run` read
  the row again after its selection, so its start could name, and replace, a
  carrier that set out in between. It now reads the Task again when that row
  names a live carrier. After both fixes: 12 Tasks, three ships each, every
  exit 0, one Graph run each. Neither fix has a fixture; the race is not
  deterministic from outside.
- Driven and working: ship on a never-run, waiting, running, landing-wait,
  done and Task-less-branch start; `auto off` mid-run stops at `land`
  ("Waiting at land: auto is off (Demo). `lf ship` lands it."); a failed land
  stops after 3 and keeps auto on; `lf -b task run` after an off waits at
  land; reopen clears auto; `task.ship` and `task.auto.off` over the socket;
  Desktop's spellings (`lf --task X ship --json`, `lf task auto off X --json`);
  `state:auto` in a Dashboard query.
- A died run with no server read "Run died at implement · no server runs here
  to start it; `lf task run cf17c1` starts it again"; with a server, "… ·
  starting again", and the pass carried it as `lf task run cf17c1` in about a
  minute.
- Upgrade from the installed 0.16.0 (`f5ff6a6c7`): a live released ship reads
  `moving · auto: on, Nov 19 10:51` and finished; a Task waiting at
  `human-review` shipped with `lf task auto on` (the captured Graph carries
  the old skill text).
- `scripts/test_desktop.sh --filter 'ServerProtocolTests|RegistryQueryTests|DTOFixtureTests|WorkModelTests|TaskStatusTests|TaskConceptTests|DesktopHeadlessTests'`:
  169 tests passed. `task_graph_launch_tests::{auto_o,a_ship,task_ship}`: 14
  passed. Clippy clean.

What QA found and left (also in `scratch/questions.md`):

- A released `lf task hold` on a Task that never took a Graph is gone after
  the upgrade (no `task_graphs` row to hold the mark), and `lf task auto off`
  on such a Task answers "auto was not on", exit 0. A later `lf -b task run`
  turns auto on and may merge.
- `--reason` is stored as a `progress` Task event and no read prints it; the
  help says "kept in the Task's history". Taken up in "Next" above.
- A Task waiting at an interactive step reads "latest Graph: Graph qaship
  finished; nothing further is launched".
- Not driven: the Desktop window (toggle and badge), iOS, a ship arriving in
  the 30 s before a carrier's Graph registers, a moved Task.

What an earlier build showed, when `lf task auto on` took up a Graph without
running it: the Task read
"Conversation: starting", "latest Graph: Starting feature", with an open
Session. A Task that took up a Graph and never ran it owes setup
(`ops/human_session/setup.rs`): its conversation starts `lf task run`
detached, and the person's own `lf task run` then met that live run and
refused. So the early take-up was itself a request to run.

Checks nobody has run since main was merged in (2026-11-19):

- Swift build and `scripts/test_desktop.sh --filter 'DTOFixtureTests|ServerProtocolTests|RegistryQueryTests|WorkModelTests|WorkNavigationTests|WaveStandupTests|WorkSelectionTests|TaskStatusTests|TaskConceptTests|DesktopHeadlessTests|PendingChangesTests|WorkCacheTests|TaskRowMetadataTests'`:
  `TaskWork.swift`, `DTOFixtureTests`, `DesktopHeadlessTests` combine `auto`
  with main's `stack_parent`. Before the merge: 285 tests, 3 failures, all the
  base's `WorkPart(rawValue: "activity")`. Gate.
- iOS (`uv run python scripts/test_ios.py`): `TaskWorkspaceView.swift` and
  `TaskListView.swift` were edited and never compiled. Gate/CI.
- `served_reads`, `machine_commands` (both edited for `task.auto`),
  `performance_tests::`, `world_recipes::`, `two_people`, the website docs
  check (`docs/dev/index.html` regeneration if it asks). CI.
- Recovery fixtures that time out here with and without this branch's edits
  ("waiting for the restart", "waiting for the Task conversation and Graph
  launches"): `a_restarted_wait_keeps_its_explicit_agent`,
  `a_shipping_tasks_restarted_wait_keeps_its_explicit_agent`,
  `a_restarted_wait_with_no_explicit_agent_ignores_the_readers`,
  `a_task_run_takes_its_waves_agent_without_flags`,
  `a_task_run_whose_carrier_died_is_carried_again_and_gives_up_after_five`.
  Gate reads what CI says; a test that fails there is fixed or deleted in this
  change.
- `a_ship_meets_a_hand_merged_pr_at_its_landing_step` and
  `a_ship_whose_run_fails_the_same_way_stops_and_keeps_auto_on` failed once on
  "read the fixture's Machine server status" beside a Swift build and passed
  at `-j 2`. Gate reads CI.

Passed before the merge (2026-11-19): storage and execution lib filters (221),
the `loopflow` filters (240 of 245, the five above), `documented_commands::`,
`binary(golden_prompt)`, clippy.

Shape, for gate's review:

- `auto off` on a Task with no Graph saves nothing and answers "auto was not
  on". The mark has no home before a Graph row exists.
- Off means two things: the person takes the interactive steps, and nothing
  merges. After `auto off`, a person's own `lf task run` past review waits at
  `pr land` until `lf ship`. Never set differs: such a run lands.
- The builtin `auto` Graph shares its name with the setting. Renaming it is
  its own Task.
- Open, nobody has decided: whether `lf -b task run` should write auto on.
- The auto setting lives in `ops/task/auto.rs` (`AutoAsk`, `ask`, `off`,
  `running_graph`, and `ship_branch_task` for a branch with no Task).
  `commands/ship.rs` is gone: `lf ship` is a `carry_task_run` call in
  `bin/lf.rs` beside `lf task run`'s. Compress 2026-11-19: clippy clean;
  storage `sqlite::task_work` 6 passed; `loopflow --lib lf::tests
  task_graph_launch_tests::auto_o …::a_ship_on_a_running` 42 passed.
- `TaskExecutionSnapshot.outlook` is on the wire and decoded in Swift, and
  no surface reads it: CLI and Desktop print `reason`. Either Desktop uses it
  the way it uses a conversation's outlook, or the field goes and `died` plus
  `reason` carry the run.
- `ship::running_graph` polls up to 30 s for a carrier's Graph to register so
  `TaskRunStart::Live` can name a Graph run. If `Live` named the carrier
  instead, the wait and its second sentence ("is running graph …" after 30 s)
  would go. Not driven: a ship arriving inside that window.

## Built: ship is a run that asks for auto

Built 2026-11-19 (concept review, second pass). `lf ship` is `carry_task_run`
with `TaskProcessOptions.auto = AutoAsk::Person { note }`; `lf -b task run`
sends `AutoAsk::Batch`. `start_task_run` (`ops/task.rs`) writes the ask once,
after its own take-up (`ship::ask`).

- A person's ask on a Task waiting at an interactive step starts at that step
  (`select_task_run`'s `stands_in`); on a live run it turns auto on and
  answers `TaskRunStart::Live` (printed as `TaskRun::Running`, `merges: true`,
  exit 0); at `end` it answers `Ended` and writes nothing.
- `ship_on`, `ShipStart`, `graph_for_ship`, `task_for_auto`, `live_graph_run`
  are deleted.
- What it taught: with the take-up in the run, two first runs at once would
  both take a Graph up, and the second's `INSERT OR REPLACE` wiped the first's
  run. A Task with no Graph now takes its first up with
  `take_up_first_task_graph` (`INSERT OR IGNORE`); the loser, and a run that
  loses `start_task_graph_run`, reads the Task again. So "moved on graph …
  while this run was starting" is gone: a bare run that loses gets "is running
  graph …", a ship reports the live run.
- A ship that meets a carrier whose Graph process has not registered waits up
  to 30 s for it (`ship::running_graph`), then falls to the bare run's
  sentence.
- Changed on purpose: a ship that fails before its run starts leaves auto
  unset. A done or abandoned Task gets the run's sentences ("is done; `lf task
  reopen` …", "is abandoned; inspect …"), not ship's own.
- Not in a fixture: two ships at the same moment. QA drives it.
- `a_ship_on_a_running_task_reports_its_run_and_starts_nothing` turns auto
  off before the ship, so the `on` it reads is the ship's, and counts one
  Graph run. Passed 2026-11-19; the ship call returned in 0.7 s.

Stored and wire value is `off` (`AutoState::Off`, `task_graph_auto.sql`,
`task_checkout.json`, `server_schema.json`, Swift `Auto.State.off`,
`docs/telemetry.md`).

Run 2026-11-19: `task_graph_launch_tests::{auto_o,a_ship,task_ship,a_stopped_ship}`
15 passed; storage `sqlite::task_work sqlite::task_summaries` 7 passed;
execution `task_graph:: ops::task::explain::tests:: store::task_work_lifecycle_tests`
14 passed; `golden_prompt` passed; `server::schema::tests:: lf::tests::
lf::commands::write::tests::` with the two auto-off fixtures, 51 passed;
`dto_fixtures::` 50 passed; `documented_commands::` 4 passed; clippy clean.

## Built: the Task run takes the conversation's outlook type

Direction 2026-11-19 (exact words in `scratch/questions.md`, "Run outlook").
Built 2026-11-19:

- `TaskExecutionSnapshot` carries `died {at_step, at}` and `outlook`
  (`SessionOutlook`), both required and nullable. `reason` is the one
  sentence, written in Rust: "Run died 12:01 at implement · starting again",
  "… · starts again 12:05", "… · started 5 times; `lf task run X` starts it
  again". `DiedRun::reason`, `next_at` and `RUN_RESTARTS` are gone.
- One rule: `DiedRun::server_start` asks `recovery::server_start` with the
  run's restarts, so a run and a conversation share the 5 starts, the
  backoff and the reasons it is left (`human_session::left_reason`). No
  server, not claimed here and not checked out here read `blocked` with the
  command; before, they read `waiting` and promised a start.
- `died.at` is null until retention records the death: a carrier found dead
  by its pid has no time, and the sentence drops the clock ("Run died at
  demo · …"). That is the usual case right after a kill.
- Fixture `task_execution_died.json` (Rust and Swift); every execution
  fixture gained the two keys; `server_schema.json` regenerated.
- Desktop prints `reason`; nothing there reads `outlook` yet.
- Also fixed: `TaskWorkView.autoLine` still named `Auto.State.stopped`, so
  the Swift package did not build since the stored word became `off`.

Run 2026-11-19: `ops::task::run_recovery::` (2), `ops::human_session::tests::`
(33), `dto_fixtures::` (51), `server::schema::tests::` (8),
`task_graph_launch_tests::a_task_run_whose_carrier_died…`,
`documented_commands::` (4), `scripts/test_desktop.sh --filter
'TaskGraphProofTests|DTOFixtureTests'` (64), clippy: all pass.

Not driven: a died run with no server, as a person would see it in
`lf task status` (QA). The unit rows cover the sentence.

## Exclusions

- Changing what `lf -b` means beyond writing auto.
- An `auto:` key in graph YAML: auto is a Task fact, not a graph property.
- `.lf/graphs/autonomous.yaml` is a graph name; it stays.
- The contract migration that drops what is leaving: LOO-971.

## Built: human-review is ship-review

Built 2026-11-19. Step and skill renamed everywhere (`code.yaml`,
`feature.yaml`, `ship-review.md`, `builtins.rs`, `task_session.md`,
`.lf/skills/task-session.md`, docs, `.lf/qa`, fixtures, tests); the page is
`scratch/ship-review.html`. The skill's goals are understanding and a decision
on every finding, with the filing bar; headless it fixes small things, files
what clears the bar and goes on. `concept-review.md` says a wrong model in the
area is this work, however large, and after build means another full pass.
`docs/orchestration.md` and `docs/getting-started.md` describe the new page.

A Task waiting at `human-review` in a stored Graph keeps that captured graph,
so its step name stays until it takes a Graph up again.

Not run here: `world_recipes::`, `local_planning` (renamed step in
assertions). CI.
