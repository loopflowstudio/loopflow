# LOO-642: Keep Auto-Ship moving through review stages and unmerged delivery

Status: implemented incl. B″, steer eb5b5f3d (explicit `lf ship` grants landing, restart is `--recover`) and B‴ docs (reference test passed 2026-10-27). LOO-622 answered its landing question in line with this one (see LOO-622 coordination). Fourth concept review (2026-10-27): no change. Integration `task_ship_*` pending at gate. Jack Heart authorized autonomous
recovery, local hotfixes and patch releases.

## Problem

Jack Heart wants an approved Task to keep moving to delivery with no manual
Workflow edits. Two observed failures stop it:

1. **Multi-route stage, no captured Workflow (LOO-633/634/636).**
   `lf ship LOO-633 --graph code` fails before launching: "demo has more than
   one route leaving it". The Task had no captured Workflow, but its history
   placed `code` at `demo`.
2. **Finished route, unmerged PR (LOO-604).** `pursue` reached `end` while
   PR #1719 is still open, and the Task's ship process keeps restarting instead
   of delivering the PR.

## Causes (verified 2026-10-27, code and read-only live store)

- **Failure 1.** The 0.14.17 release migration deleted every non-`end`
  `task_workflows` row. It says the next run takes up the default graph "placed
  at the Task's last recorded stage" (`0.14.17.001_release.sql:174`). For
  LOO-633 that is `demo`: the last move is `chose start→demo`.
  `ship_begin` (`ops/task/ship.rs:273-295`) gets its graph from
  `select_task_run` (`ops/task.rs:567`). That function also picks the single
  route `lf task run` would take, and refuses at a stage with two routes
  (`task.rs:670-685`). Ship needs the graph, not a route; its stand-in picks
  the route later, in `next_step`. So the refusal comes from reusing
  `lf task run`'s route selector, not from where the Task was placed.
- **Failure 2 is a loop of three defects.** The live store shows
  `ship_restarts=138` and 283 `lf --task LOO-604 ship` processes, each exiting 0
  within seconds:
  1. `lf -b task run LOO-604 pursue` took a route into `end`. That approves
     shipping implicitly (`choose_edge_in`, `task_work.rs:372-380`) and makes
     the run process the ship carrier. On arrival, `reach_end` requested
     completion. Completion was refused because the PR had not merged, and the
     error made the carrier exit 1.
  2. `restart_ships` restarts a dead carrier. The new `ship_begin` sees the Task
     at `end` and returns `AtEnd` without claiming the approval. Nothing is left
     to deliver the PR.
  3. The carrier stays the old failed process, so `restart_due` is
     `failed_at + backoff` measured from a failure that never moves. Once 30
     minutes have passed, every read of this Machine's Tasks starts another ship.
- Placement is the trailing move's target, even a `chose` whose Graph never
  arrived. LOO-633 is placed at `demo` although its `pursue` never recorded
  arrival. **Decision:** keep that placement. It is the accepted migration
  contract, and the `demo` stand-in reviews the work before advancing; it
  repeats `pursue` when it finds a concrete gap. Re-placing at `start` would
  replay implementation. The Task brief forbids that.

## Demo

Run in an isolated `LF_HOME` with the Claude stand-in fixture:

1. Give a Task the LOO-633 history (`code` taken up, `chose start→demo`), then
   delete its captured Workflow row the way the migration did.
   `lf -b ship INF-123 --graph code` takes up `code` at `demo`. The stand-in
   runs `demo`, chooses `deliver`, and the Task finishes. History holds the
   earlier moves plus `took_up code start→demo` and the new moves.
2. Put a Task at `end` of `pursue` with an open fixture PR, a refused completion
   request and an exited carrier. `lf task list` restarts ship once. Ship takes
   up `deliver`, lands through the fixture `gh`, and the Task completes.
   Repeated reads before then start nothing extra.

## Chosen approach

### A. Ship resolves a graph; its stand-in picks the route

- Split the take-up decision out of `select_task_run` into
  `task_graph_to_take_up(store, task, current, named) -> Option<CapturedGraph>`.
  Use the same rule both places: keep the current graph unless a different one
  is named; with none, use the Wave default, then the repository `workflow:`,
  then the builtin `code`. `select_task_run` calls it, then picks the route as
  it does today. `ship_begin` calls only the take-up. Under `lf ship`, a name
  passed with `--graph` always means a graph.
- `ship_begin` builds no route. Taking up a graph places it by history, as
  `take_up_workflow` already does. The ship loop's `next_step` then returns
  `StandIn` at `demo`.

### B. An unmerged PR at `end` is delivered, not retried

- New `ShipStep::Deliver`. `ship_next` returns it when the Workflow stands at
  `end`, the Task is not done, and its active PR is `Open` or `Publishing`.
  The ship loop takes up the `deliver` graph. It resolves through
  `CapturedGraph::load` (repository override, then builtin: gate, then
  `land --wait-and-fix`) with the note "PR #N has not merged". It is placed
  at `start`, because `end` is not a stage of `deliver`. The loop then
  continues: `start` → `Edge("deliver")` runs through
  `task_place`/`run_task_edge_for` like any other route. Arrival at `end`
  settles completion after the merge. Implementation is not replayed.
- **Nothing to deliver at `end`.** This covers a merged PR or no PR whose
  completion still fails, for example a Linear write. Ship then ends the
  approval: it unships and records a Progress event "Shipping reached end:
  <completion reason>". It does not exit and wait to be restarted. Completion
  keeps its own retry in `reconcile_delivered_task`. This replaces the bare
  `ShipStart::AtEnd` return that caused the loop.
- **Arrival inside the ship loop.** When the edge's arrival at `end` reaches
  `reach_end` and completion is refused because the PR has not merged, the ship
  loop reads `ship_next` again. If that returns `Deliver`, it records the
  refusal and continues instead of failing. Other arrival errors still fail
  the ship. `lf task run` keeps its exit status: a run that cannot complete
  still reports it.
- **Post-merge stage while at `end`.** `ship_settle_merge` skips the post-merge
  stage when the Workflow already stands at `end`. A merged PR there means
  completion, not a move back to `verify`.

### B′. Only an explicit handoff lands (Jack Heart steer, 2026-10-27)

- A route into `end` approves running that route, not merging. Draft migration
  `ship_lands` adds `tasks.ship_lands`; `ship_in` sets it only on a fresh
  approval, from `lf ship`. `choose_edge_in` passes false; existing rows default
  to 0 (uncertain history keeps no landing authority); `unship_in` clears it.
- `ship_next` offers `Deliver` only when `lands`. Otherwise `AtEnd`: `ship_end`
  settles a pending completion once, then ends the approval with "Shipping
  reached end: <reason>" plus "`lf ship <T>` hands PR #N off to land".
- A restart keeps the approval's kind; it never upgrades it.

### B″. A route-only approval carries its route and nothing else (concept review, 2026-10-27)

Implied by B′; next implement direction. No new product choice.

- **Counterexample (code read).** `lf task run T pursue` fails on its route
  into `end`; someone runs `lf task workflow move T demo`. `set_node_in` keeps
  the approval, so `restart_ships` starts `lf ship`, `next_step` returns
  `StandIn` at `demo` whatever `lands` says, and the stand-in may choose
  `code`'s `deliver`, which lands. A route-only approval merged.
- **Rule.** With `lands` false, `ship_next` offers only `Edge` (the route into
  `end` it was granted, rerun after a failure) and `AtEnd`. At any stage it
  ends the approval: "Shipping ended at <stage>: this approval ran a route,
  not a handoff; `lf ship <T>` hands it off". One check in `ship_next`, beside
  the existing `landable` read; `next_step` keeps its shape.
- **Explicit handoff grants landing (Jack Heart steer eb5b5f3d, 2026-10-27;
  supersedes "keeps its kind").** Any `lf ship <T>` grants landing, including
  to a route-only approval, with no stop-then-ship; a live carrier (ship or
  route Graph) keeps carrying and the grant is recorded as Progress
  ("Shipping may land its PR: …"). Automatic restart runs `lf ship --recover`,
  which keeps the authority it finds and never turns route/end intent into
  merge permission; `--recover` with no approval fails. `ship_lands` only
  rises (`ship_lands OR ?`) until the approval ends. `AlreadyShipping` names
  the route only for a recovery that found a live route-only carrier: "<T> is
  running <route> to its end (process X); it will not land its PR. After it
  arrives, `lf ship <T>` hands it off."
- **Docs.** `AGENTS.md` Quick Reference ("`lf ship` or any route into `end`
  approves") and the Workflow row of `docs/architecture-reference.md`
  ("choosing a route into `end` also ships it") gain: a route into `end`
  approves running it; only `lf ship` lands.
- Tests: a `next_step`/`ship_next` unit for a route-only approval at a stage
  ending it, and the existing `task_ship_delivers_an_unmerged_pr_left_at_end`
  covers the hint.

### B‴. User docs follow the accepted contract (done 2026-10-27)

`docs/orchestration.md` states that a route into `end` approves running that
route, only `lf ship` lands, and `--recover` keeps the authority it found.
`docs/lf-reference.md` regenerated: it shows `--recover`; the "Shipping grants
landing…" paragraph is long help, which the reference does not render.
Check: `reference_matches_the_compiled_cli` passed.

### LOO-622 coordination (concept review, 2026-10-27; answered by LOO-622)

LOO-622 (PR #1733, open) settled its own question: `TaskRunMode::{Stop,
InPlace, AutoShip}`, and only AutoShip (`lf -b task run` or `lf ship`) grants
landing; a plain run to `end` does not. That matches this Task's acceptance, so
nothing changes on this branch.

- If this branch lands first, LOO-622's rebase passes `lands = true` to
  `ship_task` where AutoShip approves, and `false` for the other modes
  (`choose_edge_in` already passes false).
- If LOO-622 lands first, this branch's rebase does the same mapping.
- Whichever lands second rewrites "only `lf ship` grants landing" in
  `AGENTS.md`, `docs/orchestration.md` and the `lf ship` long help to "only
  Auto-Ship (`lf ship` or `lf -b task run`)" and keeps LOO-622's
  plain-run-to-end counterexample beside `task_ship_delivers_an_unmerged_pr_left_at_end`.

### C. A restarted ship claims its approval first

- When an approval already exists, `ship_begin` claims it (`ship_task(...,
  process)`) right after the deferred, terminal and live-carrier checks. It
  does this before `require_task_launch` and before graph resolution. A
  restart that fails is then the carrier of its own failure. Its backoff is
  measured from that failure, and `ship_restarts` counts real consecutive
  failures. A first `lf ship` (no approval yet) still validates before approving.
- **Live Graph left alone.** If the Workflow is on an edge whose Process may
  still be running (`running: true`, where unknown counts as running),
  `ship_begin` records the approval with that edge's Process as the carrier and
  returns `AlreadyShipping`. When that Graph exits, `restart_ships` continues
  from where it arrived. A live ship carrier keeps `AlreadyShipping` as today.

## Source of truth

`task_workflows` and `task_workflow_moves` hold position and history (moves are
append-only). `tasks.shipped_at`, `ship_lf_process_id` and `ship_restarts` hold
the approval and its carrier. `task_prs` holds the PR phase. `ShippingSnapshot`,
`lf task status` and Desktop read these and never store them.

## Affected surfaces

- `ops/task.rs`: `select_task_run`, plus the extracted take-up helper.
  PR #1720 touches only `task_place`/`prepare_task` and PR #1719 only reads, so
  neither collides here.
- `ops/task/ship.rs`: `ship_begin`, `ship_next`/`next_step`,
  `ship_settle_merge`, a new delivery take-up, and the end-of-approval writer.
- `lf/commands/ship.rs`: the `Deliver` arm, arrival tolerance and `AtEnd`
  messaging.
- `store/sqlite/task_work.rs`: `ship_lands` column (draft `ship_lands`);
  `stop_shipping` became `end_shipping(summary)` for stop and end-of-route.
- `ShippingSnapshot.reason` reads "delivering its PR" while the delivery
  take-up stands at `start`; the landing record names #N once land starts.
  This is not a DTO field change.

## Absent and error states

- Unknown carrier or edge liveness counts as live: no claim, no launch.
- A PR that cannot be read (degraded observation) leads to no `Deliver`, and
  ship fails with the read error. Backoff then applies, measured from this
  failure.
- If `deliver` cannot be resolved, ship fails and says so. History and
  checkout are untouched.
- A busy Session in the checkout is not interrupted. Landing's own
  busy-checkout refusal fails the step, and backoff applies.

## Exclusions

- Changing `lf task run`'s exit when completion is refused at `end`.
- Re-placing history after a `chose` that never arrived (see the decision above).
- Replacing the implicit approval on a route into `end` (`choose_edge_in`).
- With an explicit `lf ship --graph X` at `end`, X is still taken up by history
  as asked.

## Delete — do not maintain

- `ShipStart::AtEnd` as a no-claim early return. It becomes "deliver or end the
  approval".
- `ship_begin`'s use of `select_task_run` and its
  `current.expect("a Task that takes nothing up keeps its graph")` branch.

## Forbidden outcomes

- Completing a Task with an unmerged PR.
- Moving a Task to `start` of a graph it already passed through just to deliver.
- Writing the live SQLite outside lf commands.
- A second approval or claim while a live Graph or ship carrier exists.
- A restart path that exits 0 while leaving the approval held by a stale failed
  carrier.

## Slices

One PR, three parts in this order:

1. C: claim first on restart.
2. A: graph-only take-up.
3. B: deliver at `end`, then end the approval.

**This slice:** all of it. The focused tests below cover each part.

## Done when

- `lf ship <T> --graph code` on a Task with no Workflow and history at `demo`
  starts the `demo` stand-in. The stand-in's decision chooses `deliver` or
  `pursue`.
- A shipping Task at `end` with an open PR takes up `deliver`, lands, and
  completes after the merge. It is never completed before the merge.
- After a failed ship, repeated reads start no more than one restart per
  backoff window, and each restart becomes the carrier.
- A Task whose edge Graph is live, or whose ship carrier is live, is left alone.
  History and the checkout are unchanged after any failed recovery.

## Acceptance checks (gate, headless)

Unit tests in `ops/task/ship.rs`: `next_step` at `end` with an open PR returns
`Deliver`; the post-merge skip at `end`.

Integration tests in `tests/linear/task_graph_launch_tests.rs` (fixture
`shipping_task`):

- `task_ship_takes_up_a_graph_at_a_multi_route_stage_and_stands_in` (failure 1)
- `task_ship_delivers_an_unmerged_pr_left_at_end` (failure 2: publish-only
  route into `end` ends shipping without landing; a fresh `lf ship` lands via a
  held repository `deliver.yaml`, merged while held)
- `task_ship_restart_that_fails_becomes_the_carrier`
- `task_ship_leaves_a_live_edge_graph_alone`

Command, with inherited `LF_*` unset:
`cargo test -p loopflow --test integration task_ship_ < /dev/null`
Expected: all `task_ship_*` tests pass. Also run `cargo clippy --all-targets -- -D warnings`.

## Live recovery (after merge and patch release)

- LOO-633/634/636 were shipped again at 12:29–12:30 UTC with `--graph
  autonomous`, and their edges are running. Leave them alone.
- LOO-604: Jack Heart stopped its loop with `lf ship --stop`; its worker is
  active. After the patched lf is installed, and once its Session is idle and
  the handoff is clear, an explicit `lf ship LOO-604` takes up `deliver` and
  lands #1719. Do not assume it is still looping.

Check: `cargo test -p loopflow --lib -- ops::task::ship:: store::sqlite::processes:: store::sqlite::task_work::` 30 passed (lands at #1718's processes.rs caller is `false`); `cargo clippy --all-targets -- -D warnings` clean after compress (2026-10-27); integration `task_ship_*` deferred to gate.
