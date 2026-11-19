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

## To build next: auto on is one action

Concept review, 2026-11-19 (`scratch/concept-review.html`), a choice made for
review. As built, "turn auto on" is two actions: `lf task auto on` sets the
mark only (and refuses a Task with no Graph), while `lf ship` and Desktop's
toggle set it and run. On a Task waiting at `human-review`, `lf task auto on`
then `lf task run` skips the step; only `lf ship` stands in. So ship is not
shorthand for anything a person can type, and the docs' "exactly `lf task
run`" is false there.

The model: auto on means nobody is waited for, so turning it on runs the Task.
`lf task auto on X`, `lf ship X` and the Desktop toggle are one operation;
`lf ship` is the short spelling (and, with no ISSUE, the one that makes a Task
for the branch).

1. `lf task auto on [ISSUE] [--reason] [--json]` runs `commands::ship::run`
   (`rust/loopflow/src/lf/mod.rs`, `lf/commands/ship.rs`, `mutation.rs`): same
   output and `--json` as `lf ship`. Live run: reported, nothing starts.
   Waiting at an interactive step: stands in there. Never run: takes up its
   Graph and runs. At `end`: reports ended.
2. Delete `ops::task::ship::auto_on` and its no-Graph refusal. On the wire one
   served operation turns auto on, `task.ship`; `task.auto` loses `on: true`
   (`lf/commands/write.rs`, `server/schema.rs`, fixtures `task_auto.json`,
   `task_auto_set.json`, Swift `DesktopActions`). Name what is left for what it
   does (off), following the wire's existing naming.
3. Tests in `rust/loopflow/tests/work/task_graph_launch_tests.rs`:
   `auto_on_starts_nothing_and_the_next_bare_run_stands_in` and
   `auto_on_a_task_that_never_ran_takes_up_no_graph_and_the_next_run_is_the_only_one`
   become one test that `lf task auto on` on a Task waiting at `review` stands
   in there and leaves one Graph run, and one that on a never-run Task it
   leaves exactly one run. Keep `a_ship_on_a_running_task_reports_its_run_and_starts_nothing`.
4. `ships` → `merges` on `TaskRun::Running` and `TaskRunStart::Run`
   (`engine/task.rs`, `ops/task.rs`, `lf/commands/graph.rs`, fixtures
   `task_shipping.json`, `task_running.json`, `server/README.md`,
   `ServerProtocolTests.swift`), printed `merges:`. "Ship" is auto on plus the
   run; this field says the run may merge, which is true with auto never set.
5. Move run recovery (`restart_lf_in`, `RUN_RESTARTS`, `DiedRun`, `died_run`,
   `restart_runs`, `continuable`, the test) from `ops/task/ship.rs` to
   `ops/task/run_recovery.rs`. It serves every Task run.
6. Docs and help: `docs/orchestration.md` Auto section (drop "starts
   nothing", the "takes `lf ship`" paragraph, "exactly `lf task run`"; `ships:`
   → `merges:`), `lf help ship` / `lf help task auto`, the operate skills'
   lines naming `lf task auto on`, `.lf/qa/auto.md`, and
   `docs/dev/machines.md` "a Task outside Auto-Ship" → "with auto not on".

Passed over: keeping `auto on` a pure setting and storing it on the Task so it
can precede a Graph, with a bare run standing in at the waiting step. That
keeps two actions and changes what a bare `lf task run` means at an
interactive step ("I finished this step"). Lost on purpose: arming auto on a
waiting Task without starting anything.

Checks: `cargo test -p loopflow --lib task_graph_launch_tests::` filtered to
the auto and ship tests, `documented_commands::`, the `write.rs` wire tests,
clippy. Swift stays with gate.

## What exists

Describes the branch at `3aaba763d0`; the section above changes who turns
auto on.

Status and setting are separate axes. `TaskState` says what is happening; the
auto setting says who takes the interactive steps. One word: **auto**.

- **The setting** is one mark on `task_graphs`: `auto` (`on` | `stopped`, NULL
  when never set), `auto_at`, `auto_by` (`Starter` JSON resolved at write
  time). Rust `Option<Auto>`, `AutoState { On, Stopped }`. It moves with the
  Graph record, survives taking up another graph, and is cleared by reopening
  the Task. Migration `task_graph_auto` adds the columns and
  `task_summaries.auto`, backfills from `task_ship_events`, NULLs
  `tasks.shipped_at` (so a released lf during upgrade skew stops relaunching
  `lf ship`), and marks `task_ship_events`, `tasks.shipped_at` and
  `task_summaries.ship_events` leaving for LOO-971.
- **Who sets it (as built):** `lf task auto on|off [ISSUE] [--reason]
  [--json]` (served as `task.auto`; `on` sets the mark and starts nothing);
  `lf ship` (on, always, the only other thing that lifts a stop);
  `lf -b task run` (on, unless stopped). Nothing else: no route
  approval, and failures, refusals and reaching `end` leave it alone.
- **`lf ship <task>`** is auto on, then exactly `lf task run` (detached, same
  output, `task.ship` returns `TaskRun`). On a live run it reports that run. On
  a Task waiting at an interactive step it stands in at that step.
- **What reads it:** the Graph runner, at each interactive step of a Task run
  (`RunningGraph::stands_in`), and the landing step, which waits only on
  `stopped` ("Waiting at pr land: auto is off (Maya). `lf ship` lands it.").
  `task_state_sql` and `TaskSection::of` never read it. `auto off` kills
  nothing: the run stops at its next interactive step or landing.
- **No `shipping` state, no Autonomous section.** A Task with auto on reads
  `moving` in Working while it runs. Rows learn the setting through the
  selection state `auto` (`state:auto`).
- **One carrier, one recovery path:** a dead run of any Task is carried again
  as `lf task run ISSUE` under `RUN_RESTARTS` (5, then it waits for a person);
  a Task that moves Machines continues the same way. The ship carrier loop,
  `restart_ship`, `ship_halt`, `ship_settle_merge`, the ship log API,
  `ShippingSnapshot`, `TaskShip`, `lf task hold`, `task.hold` and the
  `ship --stop` alias are deleted.
- **Surfaces:** `lf task status` first line `LOO-1  moving · auto: on by Maya,
  Nov 18 09:04`; snapshot field `auto`; Desktop `TaskAutoControl` toggle (on
  sends `task.ship`, off `task.auto`) and `AutoBadge` on cards and rows; docs,
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

"To build next", then gate.

QA 2026-11-19 (`scratch/qa.html`): the CLI, socket, recovery and 0.16 upgrade
paths work on a private Machine. `lf task status --json` now carries `auto`
(`TaskDetail.auto`), and a Task read from another Machine prints it; the
remote line was compiled, not driven.

Built 2026-11-19, replaced by "To build next": `lf task auto on` on a Task
with no Graph exits 1 pointing at `lf ship`.

What the reproduction that led there showed: after the early take-up the Task read
"Conversation: starting", "latest Graph: Starting feature", with an open
Session. A Task that took up a Graph and never ran it owes setup
(`ops/human_session/setup.rs`): its conversation starts `lf task run`
detached, and the person's own `lf task run` then met that live run and
refused. So the early take-up was itself a request to run.

`lf ship` still takes up the Graph a moment before its own run starts
(`graph_for_ship`), across `prepare_task`. A conversation opening inside that
window would start a second run and the ship would report the refusal. Not
seen in a fixture. Closing it means setting auto inside `start_task_run`,
after its take-up.

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

- The stored value is `stopped` while every surface says "off"
  (`auto.state: "stopped"`, `lf task auto off`). The migration is unmerged, so
  renaming the value to `off` is one edit.
- `auto off` on a Task with no Graph saves nothing and answers "auto was not
  on". The mark has no home before a Graph row exists.
- Off means two things: the person takes the interactive steps, and nothing
  merges. After `auto off`, a person's own `lf task run` past review waits at
  `pr land` until `lf ship`. Never set differs: such a run lands.
- The builtin `auto` Graph shares its name with the setting. Renaming it is
  its own Task.
- Open, nobody has decided: whether `lf -b task run` should write auto on.

## Exclusions

- Changing what `lf -b` means beyond writing auto.
- An `auto:` key in graph YAML: auto is a Task fact, not a graph property.
- `.lf/graphs/autonomous.yaml` is a graph name; it stays.
- The contract migration that drops what is leaving: LOO-971.
