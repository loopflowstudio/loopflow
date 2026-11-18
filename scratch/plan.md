# LOO-968: Ship is shorthand for auto mode on the Task's Graph

> "i feel like ship is supposed to be a small short hand that maybe updates the
> proposed graph to be auto mode, and has grown into this sprawling concept.
> lets reduce its impact on the overall system and protocol" — Jack, 2026-11-18

## Outcome

`lf ship LOO-1` turns auto mode on for the Task's Graph and then does exactly
what `lf task run LOO-1` does. There is one carrier (`lf task run`), one
recovery path (`restart_runs`), one restart rule (`RUN_RESTARTS`, 5, then wait
for a person). The landing step merges; nothing else watches the PR.

People notice:

- `lf ship` prints what `lf task run` prints (`LOO-1 · code from human-review
  running as Graph process …`, `ships:` line), and `lf task status` shows one
  Graph process, not a ship process plus its runs.
- Shipping turned on mid-run takes effect at the run's next interactive step,
  because the run reads auto mode at each step rather than at launch.
- `lf ship --stop` / `lf task hold`: the run carries on to its next interactive
  step or landing and stops there. Nothing is killed. (Made for review; see
  Decisions.)
- A Task whose run fails the same way three times stops and stays stopped; auto
  mode stays on, so the next `lf task run` keeps standing in.

## Stacks on LOO-959

LOO-959 (PR #1972, at human-review on 2026-11-18) rewrites the same code:
`restart_runs`, `RUN_RESTARTS`, `task_graphs.restarts`, ship carriers on
`task_ship_events.{pid,…}`, `processes` leaving. Building on main would mean
rewriting `restart_ship` twice and conflicting across ~300 files. Implement
starts by basing this branch on
`origin/jack/loo-959-recover-task-conversations-and-graph-runs-after` (or on
main once #1972 merges). The function names below are as they stand on that
branch (`/tmp/959-ship.rs` was read from `89943efba2`).

## Source of truth: auto mode on `task_graphs`

One fact per Task Graph row, in new nullable columns on `task_graphs`
(migration `auto_mode`, additive):

| column | meaning |
|---|---|
| `auto_on_at` | when auto mode was last turned on |
| `auto_on_by` | `Starter` JSON, resolved when it was turned on |
| `auto_off_at` | when a person turned it off; NULL while on or never off |
| `auto_off_by` | `Starter` JSON for that stop |

Rust: `AutoMode { on: Option<AutoMark>, off: Option<AutoMark> }`,
`AutoMark { at: i64, by: Starter }`, read with the graph row.
`is_on()` = `on.is_some() && off.is_none()`; `is_off()` = `off.is_some()`.

Why `task_graphs`: the brief puts it "on the Task's Graph"; it moves with the
Graph record a Task move already carries (`task_graph_record` in
`ops/human_session/moves.rs`), so "an approval that moves with a Task to
another Machine" needs no `take_in_ship_approval`. Resolving `Starter` at
write time (as a served stop already does for its person) keeps attribution
after LOO-959/LOO-965 stop keeping `processes` rows.

Taking up another graph updates the row in place and keeps the auto columns.

### Who turns it on and off

| action | writes |
|---|---|
| `lf ship` | on, unconditionally (clears off) — the only thing that lifts a stop |
| `lf -b task run`, and a run with no interactive step ahead (`ships`) | on, unless off is set (today's `Approval::Route`) |
| `lf ship --stop`, `lf task hold`, `task.hold`, Desktop Stop Auto-Ship | off, with the stopper |
| anything else (failure, refusal, unassign, reaching `end`) | nothing |

"Approval never granted without a person's `lf ship` or `-b`" holds: a route is
a person's run with nothing left for a person.

### What reads it

- **Graph runner** (`lf/commands/graph.rs`, `RunningGraph`): at each
  interactive step of a Task run, read auto mode. On → run it headless,
  standing in. Not on → block there as `Stop` mode does today (the `-i`
  in-place mode is unchanged). `stopped_landing` reads `is_off()` in place of
  `task_ship_stop`. This replaces the launch-time `stop_at_interactive` flag for
  Task runs; plain `lf graph run` keeps its flag.
- **Task state** (`task_state_sql`, `sqlite/durable.rs`): `shipping` = a live
  run (or open Graph process) and `auto_on_at IS NOT NULL AND auto_off_at IS
  NULL`; else `moving`. Auto on, nothing live → `idle` (not `active`: no person
  is needed). The ship-event carrier subquery goes. `TaskState::Shipping` and
  `TaskSection::Autonomous` stay as derived labels; every surface already reads
  them and they now mean "auto mode plus a running Graph".
- **Task snapshot** (`ops/task/snapshot.rs`, `TaskExecutionSnapshot`):
  `auto: AutoMode` replaces `ship_approval` and `ship_stop` (both `ShipMark`).
- **`lf task status`** (`bin/lf.rs:744-760`): one "auto mode: on by Maya
  2h ago" / "auto mode: off, stopped by Maya" line from `auto`.
- `task_details.rs:118`, `task_started.rs:39`: `shipped_at` → `auto_on_at`.

## What goes

Execution (`ops/task/ship.rs`, about 1,000 lines → `ship_on`, `ship_off`,
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
  exclusion, `local_shipping_tasks`. A dead run of an auto Task is carried
  again as `lf task run ISSUE` under `RUN_RESTARTS`; the run stands in by
  itself. "A ship never gives up" goes.
- `ship_halt` and `TaskRunStopped`'s call to it: a terminal stop leaves auto
  mode alone.
- `withdraw`'s process kill. Kept: clearing GitHub auto-merge Loopflow armed
  (`clear_task_pr_merge_before_head_mutation`) when auto goes off.
- `TaskShip` (`engine/task.rs`): `task.ship` returns `TaskRun`. `Hold`
  stays (it says whether auto was on).

Storage (`sqlite/task_work.rs`): `TaskShipping`, `ShipStop`, `ShipEvent`,
`ship_log_in`, `append_ship_event_in`, `shipping_in`, `ship_in`, `unship_in`,
`ship_task`, `stop_shipping`, `task_shipping`, `task_ship_stop`,
`task_ship_approval`, `take_in_ship_approval`, `count_ship_restart`,
`reset_ship_restarts`, `SHIP_RESTARTS_COUNTED`, the ship part of
`start_run_in`, and their tests in `task_work.rs`, `query_plans.rs`,
`processes.rs`, `task_work_lifecycle_tests.rs`.

Wire: `ship_approval`, `ship_stop` → `auto`; `ShippingSnapshot` and roadmap
`shipping` (`lf/commands/waves.rs:499`); `TaskShip`. Fixtures:
`task_checkout.json`, `local_task.json`, `closed_task_page.json`,
`roadmap_snapshot.json`, `server_schema.json`, `task_lifecycle.json`,
`desktop_inspection.json`. Swift: `TaskWork.shipApproval/shipStop` → `auto`;
`WaveWorkMap.ShippingSnapshot` goes; `TaskWorkView` shows Stop Auto-Ship when
`auto` is on and takes its reason line from the Task's `reason`;
`MockWaveFixture` drops `"shipping":null`.

Kept: `ship_control` (`TaskRunControl`, whether ship is offered),
`lf ship` from a bare branch (`ship_branch_task`), `TaskState::Shipping`,
`TaskSection::Autonomous`, the `state:shipping` / `autonomous` selection
states.

### Schema staging (released v0.16.0 has `task_ship_events` and `tasks.shipped_at`)

- `auto_mode.sql` (expand): add the four `task_graphs` columns; backfill
  `auto_on_*` from the latest `approved` event and `auto_off_*` from a later
  `stopped`, resolving the starter from the event's process/person; add the
  columns to the `task_graphs` planning update trigger's `WHEN` clause
  (`planning_update_triggers_compare_every_column`).
- Same migration: `UPDATE tasks SET shipped_at=NULL`. A released lf still
  running on the Machine during upgrade skew otherwise sees every shipped Task
  as "ship process exited" and relaunches `lf ship` against the new store.
  Released lf then reads no Task as shipping; that is the honest answer.
- `-- leaving: task_ship_events LOO-968`, `-- leaving: tasks.shipped_at LOO-968`,
  and the six `tasks.ship_*` columns already marked by `task_machine_local`.
  File the contract Task in the same change. `SCHEMA.md` updated.
- `WorkValue::TaskShippedAt` becomes `HistoricalTaskShippedAt`: journals
  written by released lf still parse; nothing writes it.
- The Move JSON keeps its `approval` key, filled from `auto.on`, for a
  released receiver; marked with the contract Task.

## Acceptance check at gate

One headless fixture in `rust/loopflow/tests/work/task_graph_launch_tests.rs`
with a fake `gh` and a feature graph (start → implement → human-review →
land → verify):

1. Task waits at `human-review`. `lf --task T ship` → returns a `TaskRun` with
   `ships: true`; `lf task status T --json` shows `auto.on.by` = the person,
   state `shipping`, one Graph process.
2. `lf task wait T` → the run stood in at `human-review`, the land step merged
   the fake PR, verify ran headless, state `done`.
3. Second Task: `lf ship`, then `lf ship --stop` while implement runs → the run
   reaches `human-review` and blocks there (state `active`), `auto.off.by` =
   the person, no merge. `lf -b task run T` → stops before `land` with "not
   approved to merge"; auto stays off. `lf ship T` → lands.
4. Kill the carrier of an auto Task mid-step; one recovery pass relaunches
   `lf task run T` (not `lf ship`); it carries on standing in.

The existing ship tests become this: `task_ship_stop_withdraws_the_approval_…`,
`task_ship_stop_sticks_through_a_headless_relaunch`,
`task_ship_stops_shipping_when_its_run_fails_…` (now: auto stays on, run stops).

## Slices

1. **Auto mode is live in a run** (teaches the most: can the runner decide
   stand-in per step, and does `lf ship` collapse to task run?). Migration
   columns + `AutoMode` read/write; runner reads it at interactive steps and at
   landing; `lf ship` = `ship_on` + `carry_task_run` (bare returns once registered, `-b` waits; the run stands in because auto is on, not because of its mode);
   `-b`/route write on-unless-off. Delete the carrier loop, `ShipStep`,
   `ShipMerge`, `TaskShip`. Shows: step 1–3 of the acceptance check with one
   Graph process. If the runner can't switch mid-run cleanly, the fallback is
   reading auto mode once per `drive` and relaunching at the interactive step;
   say which in the plan when it lands.
2. **State and wire**: `task_state_sql`, snapshot `auto`, status line, roadmap
   `shipping` gone, Swift and fixtures, moves carry auto in the graph record.
3. **One recovery**: delete `restart_ship`, `ship_halt`, shipping exclusions;
   acceptance step 4.
4. **Store leaves**: stop all `task_ship_events` / `shipped_at` writes, delete
   the storage API listed above, backfill + NULL `shipped_at`, leaving marks,
   contract Task, docs (`docs/surfaces.md`, `docs/dev/reference*.md`,
   `server/README.md`, iOS README).

## Exclusions

- Changing what `lf -b` means beyond writing auto mode.
- Graph definition changes (no `auto:` key in graph YAML; auto mode is a Task
  fact, not a graph property).
- Desktop affordances beyond swapping the shipping fields.

## Decisions made for review

- **Stop doesn't kill.** `--stop` turns auto off and the run stops at its next
  interactive step or landing. Passed over: killing the run (today's
  behavior), which needs a carrier identity ship had and `lf task run` handles
  through `lf task interrupt`.
- **Hand-merged PRs go through the Graph.** `ship_settle_merge` goes; the land
  step sees the merge. Passed over: a PR watcher in the runner.
- **Failures don't touch auto mode.** Passed over: `ship_halt`'s withdrawal,
  which made a Task silently stop shipping.
- **Clear `tasks.shipped_at` in the migration** so released lf stops
  relaunching ships during skew.
