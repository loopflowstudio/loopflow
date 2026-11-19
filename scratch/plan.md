# LOO-968: Ship is shorthand for Auto-Ship on the Task's Graph

> "i feel like ship is supposed to be a small short hand that maybe updates the
> proposed graph to be auto mode, and has grown into this sprawling concept.
> lets reduce its impact on the overall system and protocol" — Jack, 2026-11-18

## Outcome

`lf ship LOO-1` turns Auto-Ship on for the Task's Graph and then does exactly
what `lf task run LOO-1` does. There is one carrier (`lf task run`), one
recovery path (`restart_runs`), one restart rule (`RUN_RESTARTS`, 5, then wait
for a person). The landing step merges; nothing else watches the PR.

"Auto mode" in the quote is the thing the product already calls **Auto-Ship**
(Desktop's Auto-Ship / Stop Auto-Ship buttons, the Notice, docs, every operate
skill). The fact keeps that one name in code, columns, wire and output; no
"auto mode" synonym is added. (Concept review, made for review.)

People notice:

- `lf ship` prints what `lf task run` prints (`LOO-1 · code from human-review
  running as Graph process …`, `ships:` line), and `lf task status` shows one
  Graph process, not a ship process plus its runs.
- Auto-Ship turned on mid-run takes effect at the run's next interactive step,
  because the run reads it at each step rather than at launch.
- `lf task hold`: the run carries on to its next interactive
  step or landing and stops there. Nothing is killed. (Made for review; see
  Decisions.)
- A Task whose run fails the same way three times stops and stays stopped;
  Auto-Ship stays on, so the next `lf task run` keeps standing in.

## Stacks on LOO-959

LOO-959 (PR #1972, open and unmerged on 2026-11-18, head `89943efba2`) rewrites
the same code: `restart_runs`, `RUN_RESTARTS`, `task_graphs.restarts`, ship
carriers on `task_ship_events.{pid,…}`, `processes` leaving. Building on main
would mean rewriting `restart_ship` twice and conflicting across ~300 files.
Implement starts by basing this branch on
`origin/jack/loo-959-recover-task-conversations-and-graph-runs-after` (or on
main once #1972 merges). Function names below are as they stand at
`89943efba2`.

## Source of truth: one Auto-Ship mark on `task_graphs`

Auto-Ship is a three-way fact: never set, on, or stopped. It is one mark, not
a pair, in three new nullable columns on `task_graphs` (migration
`task_graph_auto_ship`, additive):

| column | meaning |
|---|---|
| `auto_ship` | `'on'` or `'stopped'`; NULL when nobody has set it |
| `auto_ship_at` | when it was last set |
| `auto_ship_by` | `Starter` JSON, resolved when it was set |

Rust: `Option<AutoShip>` read with the graph row, where
`AutoShip { state: AutoShipState, at: i64, by: Starter }` and
`AutoShipState { On, Stopped }` (`#[non_exhaustive]` not needed: two states is
the concept). Who turned it on before a stop is history nobody reads; the
current mark is the whole fact.

Why `task_graphs`: the brief puts it "on the Task's Graph"; it moves with the
Graph record a Task move already carries (`task_graph_record` in
`ops/human_session/moves.rs`), so "an approval that moves with a Task to
another Machine" needs no `take_in_ship_approval`. Resolving `Starter` at
write time (as a served stop already does for its person) keeps attribution
after LOO-959/LOO-965 stop keeping `processes` rows. `task_state_sql` already
needs the local `run_pid` to say `shipping`, so moving the fact off the shared
`tasks` row loses no cross-Machine answer.

Taking up another graph updates the row in place and keeps the Auto-Ship
columns.

### Who sets it

| action | writes |
|---|---|
| `lf ship` | `on`, unconditionally — the only thing that lifts a stop |
| `lf -b task run` | `on`, unless `stopped` |
| `lf task hold`, `task.hold`, Desktop Stop Auto-Ship | `stopped`, with the stopper |
| anything else (a route with no interactive step ahead, reaching `end`, failure, refusal, unassign) | nothing |

The route write (`Approval::Route`, "its run reached the end") goes, and the
`Approval` enum with it. Nothing reads `on` except the runner at an
interactive step, and a run with no interactive step ahead never reaches one;
at `end` nothing is left to read it. Such a run still merges, because landing
waits only on `stopped`, and `lf task hold` still stops it. The `ships:` line
on `lf task run` stays: it is computed from the Graph, not stored. What
changes is the label: that run shows `moving` (Working), not `shipping`
(Autonomous), because nobody turned Auto-Ship on. (Concept review, made for
review.)

"Approval never granted without a person's `lf ship` or `-b`" now holds
literally.

### What reads it

- **Graph runner** (`lf/commands/graph.rs`, `RunningGraph`): at each
  interactive step of a Task run, read Auto-Ship. `on` → run it headless,
  standing in. Otherwise → block there as `Stop` mode does today (the `-i`
  in-place mode is unchanged). `stopped_landing` reads `stopped` in place of
  `task_ship_stop`, and its message stays "not approved to merge (Maya stopped
  shipping). `lf ship` lands it." This replaces the launch-time
  `stop_at_interactive` flag for Task runs; plain `lf graph run` keeps its
  flag.
- **Task state** (`task_state_sql`, `sqlite/durable.rs`): `shipping` = a live
  run (or open Graph process) and `auto_ship = 'on'`; else `moving`. Auto-Ship
  on, nothing live → `idle` (not `active`: no person is needed). The
  ship-event carrier subquery goes. `TaskState::Shipping` and
  `TaskSection::Autonomous` stay as derived labels; every surface already reads
  them and they now mean "Auto-Ship on plus a running Graph".
- **Task snapshot** (`ops/task/snapshot.rs`, `TaskExecutionSnapshot`):
  `auto_ship: Option<AutoShip>` replaces `ship_approval` and `ship_stop` (both
  `ShipMark`).
- **`lf task status`** (`bin/lf.rs:744-760`): one line, "Auto-Ship: on by Maya,
  Nov 18 09:04" or "Auto-Ship: stopped by Maya, Nov 18 09:10"; a mark that
  names nobody (backfilled from released rows) drops "by".
- `task_details.rs:118`, `task_started.rs:39`: `shipped_at` → `auto_ship_at`
  where `auto_ship = 'on'`.
- **Task summaries** (`sqlite/task_summaries.rs`): `ship_events` counted
  `task_ship_events` kinds per ended Task (`docs/telemetry.md:60`). New
  summaries write the final Auto-Ship state into a new nullable
  `task_summaries.auto_ship` column; `ship_events` is marked leaving and new
  rows write `{}`.

## What goes

Execution (`ops/task/ship.rs`, about 1,000 lines → `ship_on`, `ship_stop`,
`ship_branch_task`, `starter`, maybe 150):

- The carrier loop in `lf/commands/ship.rs` (`ship_next`, `ShipStep`,
  `merged`, `halt`, the per-iteration `task_place`), `ship_begin`'s
  carrier claim, `ShipStart::AlreadyShipping`.
- `ship_settle_merge`, `ShipMerge`, `post_merge_step`: a PR someone merges by
  hand is met by the Graph's own landing step (`lf pr land` on a merged PR
  returns success) and then verify. Made for review.
- `ShippingSnapshot`, `ShippingState`, `shipping_snapshot`, `read_shipping`,
  `carrier_evidence`, `carrier_failure`, `restart_due`, `ship_progressed`
  and their tests. The run's own status line (`DiedRun::reason`, the landing,
  limit waits) already says what the "Restarting/Landing/Repairing" states
  restated.
- `restart_ship`, the shipping loop in `restart_runs`,
  `died_run`'s `task_shipping` exclusion, `restart_waiting_runs`'s shipping
  exclusion, `local_shipping_tasks`. A dead run of an Auto-Ship Task is carried
  again as `lf task run ISSUE` under `RUN_RESTARTS`; the run stands in by
  itself. "A ship never gives up" goes.
- `ship_halt` and `TaskRunStopped`'s call to it: a terminal stop leaves
  Auto-Ship alone.
- `withdraw`'s process kill. Kept: clearing GitHub auto-merge Loopflow armed
  (`clear_task_pr_merge_before_head_mutation`) when Auto-Ship is stopped.
- `TaskShip` (`engine/task.rs`): `task.ship` returns `TaskRun`. `Hold`
  stays (it says whether Auto-Ship was on).
- Ship carriers elsewhere: the `ship_carrier` arm of retention's process
  union (`sqlite/retention.rs:255`), the `task_ship_events` exit update in
  `sqlite/graph_inventory.rs:763`, the `task_ship_events` line of
  `graph_process.rs:33`'s doc.

Storage (`sqlite/task_work.rs`): `TaskShipping`, `ShipStop`, `ShipEvent`,
`Approval`, `ship_log_in`, `append_ship_event_in`, `shipping_in`, `ship_in`,
`unship_in`, `ship_task`, `stop_shipping`, `task_shipping`, `task_ship_stop`,
`task_ship_approval`, `take_in_ship_approval`, `count_ship_restart`,
`reset_ship_restarts`, `SHIP_RESTARTS_COUNTED`, the ship part of
`start_run_in`, and their tests in `task_work.rs`, `query_plans.rs`,
`processes.rs`, `task_work_lifecycle_tests.rs`, and the
`DROP TABLE task_ship_events` fixture in `lf/commands/waves.rs:4842`.

Wire: `ship_approval`, `ship_stop` → `auto_ship`; `ShippingSnapshot` and
roadmap `shipping` (`lf/commands/waves.rs:499`); `TaskShip`. Fixtures:
`task_checkout.json`, `local_task.json`, `closed_task_page.json`,
`roadmap_snapshot.json`, `server_schema.json`, `task_lifecycle.json`,
`desktop_inspection.json`. Swift: `TaskWork.shipApproval/shipStop` →
`autoShip`; `WaveWorkMap.ShippingSnapshot` goes; `TaskWorkView` shows Stop
Auto-Ship when `autoShip` is on and takes its reason line from the Task's
`reason`; `MockWaveFixture` drops `"shipping":null`.

Kept: `ship_control` (`TaskRunControl`, whether ship is offered),
`lf ship` from a bare branch (`ship_branch_task`), `TaskState::Shipping`,
`TaskSection::Autonomous`, the `state:shipping` / `autonomous` selection
states.

### Schema staging (released v0.16.0 has `task_ship_events` and `tasks.shipped_at`)

- `task_graph_auto_ship.sql` (expand): add the three `task_graphs` columns and
  `task_summaries.auto_ship`; backfill from each Task's latest `approved` or
  `stopped` event (`withdrawn` counts as `stopped`), resolving the starter from
  the event's process/person; add the columns to the `task_graphs` planning
  update trigger's `WHEN` clause
  (`planning_update_triggers_compare_every_column`).
- Same migration: `UPDATE tasks SET shipped_at=NULL`. A released lf still
  running on the Machine during upgrade skew otherwise sees every shipped Task
  as "ship process exited" and relaunches `lf ship` against the new store.
  Released lf then reads no Task as shipping; that is the honest answer.
- `-- leaving: task_ship_events LOO-968`, `-- leaving: tasks.shipped_at LOO-968`,
  `-- leaving: task_summaries.ship_events LOO-968`, and the six `tasks.ship_*`
  columns already marked by `task_machine_local`. File the contract Task in the
  same change. `SCHEMA.md` updated.
- `WorkValue::TaskShippedAt` becomes `HistoricalTaskShippedAt`: journals
  written by released lf still parse; nothing writes it.
- The Move JSON keeps its `approval` key, filled when `auto_ship = 'on'`, for a
  released receiver; marked with the contract Task.

## Acceptance check at gate

One headless fixture in `rust/loopflow/tests/work/task_graph_launch_tests.rs`
with a fake `gh` and a feature graph (start → implement → human-review →
land → verify):

1. Task waits at `human-review`. `lf --task T ship` → returns a `TaskRun` with
   `ships: true`; `lf task status T --json` shows `auto_ship.state` = `on`,
   `auto_ship.by` = the person, state `shipping`, one Graph process.
2. `lf task wait T` → the run stood in at `human-review`, the land step merged
   the fake PR, verify ran headless, state `done`.
3. Second Task: `lf ship`, then `lf task hold` while implement runs → the run
   reaches `human-review` and blocks there (state `active`), `auto_ship` =
   `stopped` by the person, no merge. `lf -b task run T` → stops before `land`
   with "not approved to merge"; Auto-Ship stays stopped. `lf ship T` → lands.
4. Kill the carrier of an Auto-Ship Task mid-step; one recovery pass relaunches
   `lf task run T` (not `lf ship`); it carries on standing in.
5. Third Task at `demo` of a graph with no interactive step after it: bare
   `lf task run T` → `ships:` line printed, state `moving`, `auto_ship` null,
   the land step merges.

The existing ship tests become this: `task_ship_stop_withdraws_the_approval_…`,
`task_ship_stop_sticks_through_a_headless_relaunch`,
`task_ship_stops_shipping_when_its_run_fails_…` (now: Auto-Ship stays on, run
stops).

## Slices

Implement (2026-11-18) built slices 1–4 in one pass on top of LOO-959; they
could not land apart because the state SQL, the runner and the stored log
shared every reader.

Built:

- `task_graph_auto_ship` migration (named to apply after LOO-959's
  `process_rows_on_owners`): three `task_graphs` columns,
  `task_summaries.auto_ship`, backfill, `shipped_at` cleared, leaving marks for
  LOO-971 (the contract Task). SCHEMA.md regenerated.
- Storage: `AutoShip`/`AutoShipState`, `set_task_auto_ship` (with
  `keeps_stop` for `-b`), `task_auto_ship`; the ship log API, `Approval`,
  the route write and `RunOwner::ShipCarrier` deleted. Take-up and moves keep
  the mark; reopening clears it.
- Runner: `GraphRunner::stopping_at_interactive` is gone; `RunningGraph::stands_in`
  reads Auto-Ship at each interactive step (the runner read it once per step,
  so no fallback was needed), `stopped_landing` reads `stopped`.
  `SkillOutcome::Unapproved` became `StopsAt`.
- `lf ship` = `ship_on` + `carry_task_run` (stands in from the waiting step);
  `task.ship` returns `TaskRun`; `TaskShip` deleted. `-b task run` turns it on
  unless stopped; `start_task_run` runs through interactive steps while it is on.
- Recovery: one loop, `lf task run` for every dead run; `ship_halt`,
  `restart_ship`, the shipping exclusions and `local_shipping_tasks` deleted.
- Wire: `TaskSnapshot.auto_ship`; roadmap `shipping` gone (`ship_control`
  says "Auto-Ship is already on"); fixtures, server schema, Swift
  (`TaskExecution.autoShip`, Stop Auto-Ship reads it), dev docs.
- `lf task status`: "Auto-Ship: on by X, <local time>" / "Auto-Ship: stopped by X, …".
- QA (2026-11-18): `lf ship` now detaches as `lf task run` does, and on a
  live run reports that run (`ships: true`) instead of the run refusal;
  `scratch/qa.html`.

Left for gate:

- The acceptance fixture as one test (steps 1–5 above). Today the pieces are
  separate tests in `task_graph_launch_tests.rs`:
  `a_bare_ship_stands_in_because_auto_ship_is_on`,
  `a_stopped_ship_kills_nothing_and_its_run_carries_on`,
  `task_ship_stop_sticks_through_a_headless_relaunch`,
  `a_ship_whose_run_fails_the_same_way_stops_and_keeps_auto_ship_on`,
  `a_ship_meets_a_hand_merged_pr_at_its_landing_step`.
- Checks run here (2026-11-18): storage `task_work::`, `task_summaries::`,
  `query_plans::` pass; execution runner/ship/lifecycle/engine/wire pass;
  `task_graph_launch_tests::` ship, stop, batch and restart rows (78) pass;
  `dto_fixtures::`, `server::schema::`, `write::tests::`, `served_reads`,
  `documented_commands::` pass; clippy clean. Swift
  `DTOFixtureTests|DesktopHeadlessTests|ServerProtocolTests|WorkModelTests|TaskConceptTests`:
  all edited tests pass.
- Not from this branch, seen on the LOO-959 base: `world_recipes::one_task_waits_at_human_review_after_planning_and_building`
  (status now has a `conversation` key, 33ab7ff469), and three Swift
  "Work observation stream" tests failing on `WorkPart(rawValue: "activity")`.
- `docs/dev/index.html` regeneration if the website check asks.

## Exclusions

- Changing what `lf -b` means beyond writing Auto-Ship.
- Graph definition changes (no `auto:` key in graph YAML; Auto-Ship is a Task
  fact, not a graph property).
- Desktop affordances beyond swapping the shipping fields.
- Renaming the board's Autonomous section or the `shipping` state (see the
  review page's question 3).

## Decisions made for review

- **Stop doesn't kill.** `lf task hold` sets `stopped` and the run stops at its next
  interactive step or landing. Passed over: killing the run (today's
  behavior), which needs a carrier identity ship had and `lf task run` handles
  through `lf task interrupt`.
- **Hand-merged PRs go through the Graph.** `ship_settle_merge` goes; the land
  step sees the merge. Passed over: a PR watcher in the runner. Graphs with
  no land step (`build`, `pursue`, `deep-clean`, `queue`) relied on
  `ship_settle_merge` to complete a hand-merged Task; their run now finishes
  its steps into `end` and completion settles. No merge check between steps
  was added.
- **Failures don't touch Auto-Ship.** Passed over: `ship_halt`'s withdrawal,
  which made a Task silently stop shipping.
- **Clear `tasks.shipped_at` in the migration** so released lf stops
  relaunching ships during skew.
- **One name: Auto-Ship.** Passed over: "auto mode" from the quote, which
  would be a fourth name beside ship, Auto-Ship and shipping.
- **One mark, not an on/off pair.** Passed over: keeping who turned it on after
  a stop, which nothing reads.
- **No route approval.** A run with no interactive step ahead merges without
  writing Auto-Ship and shows `moving`. Passed over: keeping the write so it
  shows `shipping`/Autonomous, a stored label for a fact the Graph already
  answers.

## Docs (write-docs, 2026-11-18)

Written in place: `docs/orchestration.md` (new `### Auto-Ship` section, state
table), `docs/ops.md`, `docs/getting-started.md`, `docs/sessions.md`,
`docs/troubleshooting.md`, `docs/work.md`, `docs/telemetry.md`, and the
Auto-Ship line in `task_operate.md`, `repo_operate.md`, `wave_operate.md`,
`launch-plan.md`. Slice 4's doc list is down to `docs/dev/*`, `server/README.md`
and the iOS README.

`lf ship --stop` doesn't exist on main; `lf task hold` is the one stop.

## Build pass after human review: one word for Auto-Ship (in scope)

The state `shipping`, the board section `Autonomous` and the mark Auto-Ship are
one idea under three words. This pass makes Auto-Ship the only word:

- `TaskState::Shipping` → `TaskState::AutoShip` (label `auto-ship`) and
  `TaskSection::Autonomous` → `TaskSection::AutoShip` (title "Auto-Ship"),
  through `task_state_sql`, the wire DTOs and their fixtures in
  `tests/fixtures/dto/`, Swift, Desktop, CLI output, docs, AGENTS.md's work
  model and the operate skills. No compat alias: nothing has shipped.
- The stop's words follow (concept review question 1a): `lf task hold` keeps
  its name and prints "LOO-12: Auto-Ship stopped" / "Auto-Ship was not on";
  the run waits with "Auto-Ship stopped by Maya. `lf ship` lands it." Tests
  call `task hold`, not the `ship --stop` alias, and the alias goes.
- `.lf/graphs/autonomous.yaml` is a graph name, not the section; leave it.
- Still open, not part of this pass: whether `lf -b task run` writes Auto-Ship
  on (concept review question 2).
