# LOO-687 · Stopping ship sticks

Status: accepted (Jack asked to launch, 2026-10-29); implemented except the
remaining work below. Assumptions made while implementing are in
`scratch/questions.md`.

## Problem

Jack stopped shipping manabot Task 892c47 four times (2026-10-28 16:56 →
2026-10-29 09:33). Each time its Task Session relaunched pursue with
`lf task run 892c47 --from compress|implement --reason …`. `start_run_in`
called `ship_in` because the rest of the graph had no interactive step, so the
Task read shipping again and recovery could restart its ship process. Jack's
stop left nothing later runs respected. The agent's `--reason` was saved as a
steer by `Person { name: "Jack Heart" }`, so it read as Jack's words.

## Demo

On a real Task with an all-headless remainder (`auto`, or `pursue`):

```
lf ship LOO-x            # Shipping: Jack Heart shipped it
lf ship --stop LOO-x     # Stopped shipping: Jack Heart
# Task Session (or you) relaunches:
lf task run LOO-x --from implement --reason "restart after crash"
lf task status LOO-x
```

Status shows the Task `moving`, not `shipping`, with
`Not shipping: Jack Heart stopped it Oct 29 09:33 · lf ship approves it again`,
the run `started by Session "LOO-x task" for Jack Heart`, and the reason comment
authored by that Session. On `auto`, the run implements and publishes, then
waits at `land` instead of merging. `lf ship LOO-x` lands it.

## Decision: where ship intent lives

Two options were offered. Jack reverted a two-tier approval on 2026-10-27
(#1741, LOO-648), restoring "`lf ship` or any route into `end` approves the
whole run through landing." That decision holds. A person's stop now limits
it:

- **One approval, plus a durable stop.** `lf ship --stop` records who stopped it
  and when. A route into `end` approves Auto-Ship only when no stop stands.
  Only an explicit `lf ship` (the Desktop Auto-Ship button runs it) clears the
  stop and approves again. This is not a second grant: the approval stays the
  one `shipped_at`, and the stop only blocks implicit re-approval.
- **An unapproved run never lands.** `land` is the real merge authority: it is
  a graph step, not `shipped_at`. Before #1741 every run that reached `land` was
  approved by construction. A stop breaks that, so the runner checks approval
  when it reaches a landing step. Without approval, the run stops there, and the
  Task waits at that step the way it waits at an interactive one. `lf ship`
  continues from it. The check is made when the step is reached, not at
  launch, so it also covers a stop made mid-run.
- `ship_halt` (Loopflow withdrawing after identical repeated failures) does not
  record a stop. Its cause was the failure, and a relaunch that fixes it may
  approve again by reaching `end`.

Rejected: **explicit-only approval** (delete ship-on-run entirely). It's
simpler, but it reverses #1741's accepted behavior, under which `auto` and
`-b task run` land without `lf ship`. It would also still need the landing check,
because the graph's `land` step merges whether or not the Task is shipping.

## Source of truth

`tasks` row: `shipped_at` (approval, existing), plus new
`ship_stopped_at INTEGER`, `ship_stopped_lf_process_id TEXT`,
`ship_approved_lf_process_id TEXT`. Approval and stop are mutually exclusive:
`ship_in(explicit)` clears the stop, and `stop_shipping` sets it while clearing
`shipped_at`. Both are written through `planning_write::local` WorkEdits
(`TaskShipStoppedAt`, as `TaskShippedAt` is), so a Task move carries them.
Who started what is **derived** from `processes`, never stored as names:

`process_starter(process) -> Starter { Person(name) | Session { id, name, requester } | Loopflow { command } }`.
Walk self → `parent_lf_process_id`. The first process with `caller_session_id`
gives the Session. A root with no Session is the person: the participant name,
or `attribution unresolved`. A process with a parent but no Session anywhere in
its chain was started by Loopflow. For example, `restart_ships` from
`lf task status` inherits `LF_PROCESS_ID`.

## Changes

1. **Migration** `uv run python scripts/new_migration.py ship_stop`: the three
   columns. Add a `store_revision_tasks_update` coverage check if the trigger
   lists columns.
2. **Store** (`store/sqlite/task_work.rs`): `ship_in` takes
   `Approval::{Explicit, Route}`. A `Route` approval writes nothing and returns
   `None` when `ship_stopped_at` is set. The first time a given run is skipped,
   record one Progress event: `Not shipping: <starter> stopped it`. `Explicit`
   clears the stop. `stop_shipping` records the stop with the stopping process.
   The callers are `start_run_in`, `arrive_in`, `end_task_graph` (Route) and
   `ship_task` (Explicit). `TaskShipping` gains `approved_by: Option<LfProcessId>`;
   new `task_ship_stop(task) -> Option<(i64, Option<LfProcessId>)>`.
3. **Landing boundary** (`task_graph.rs`, `lf/commands/graph.rs` runner):
   `CapturedGraph::is_landing(place)` is true for a Command step resolving to
   `lf land`/`lf pr land` through the shared Clap navigation, or for skill
   `pr-land`. Before running a landing step for a Task, the runner reads
   `task_shipping`. If the Task is not approved, it arrives at that step's
   name and exits 0 with
   `Waiting at land: not approved to merge (Jack Heart stopped shipping). lf ship LOO-x lands it.`.
   `select_task_run` accepts `At { step }` naming a landing step: start *at* it.
   `TaskGraphPosition::At` docs and `ship::next_step` follow.
4. **Attribution** (`ops/task.rs::append_task_comment`): if the comment has
   agent provenance, a steer gets author
   `TaskCommentAuthor::Session { session_id, requester }`. The body carries
   `<!-- loopflow-session:<id> -->` beside the requester marker, so Linear
   round-trips it (`linear/mod.rs` `From<&LinearComment>`). `comment_steer_in`
   maps it to new `durable::Author::Session(LfSessionId)`, whose text is
   `Comment X by Session "<name>" for Jack Heart`. Swift
   `TaskCommentAuthor.session(id:requester:)`, `TaskCommentsView`, the
   `bin/lf.rs` comment list, and DTO fixtures all change too. `ship_begin`'s
   handoff note and `ship_stop`'s note use `process_starter` instead of
   `participant_or`.
5. **Status**: text `lf task status` prints `started by …` on each Graph and
   run line, `Auto-Ship approved by … <time>`, and the standing stop line.
   JSON: Graph inventory entry gains `started_by: Starter`. `ShippingSnapshot`
   gains `approved_by: Starter`. The Task detail gains
   `ship_stop: Option<ShipStop { at, by: Starter }>`. These are DTOs: required or
   explicit Optional fields, with Rust and Swift fixtures. Desktop's
   TaskWorkView shows the stop line where the shipping reason goes, plus
   `started by` on the run row.
6. **Skills/docs**: `task_session.md` and `repo_session.md` say a relaunch
   continues the work and never re-ships a Task someone stopped; run `lf ship`
   only when that person asks. Update the AGENTS.md quick-reference ship
   sentence, the `ship.rs` module doc, and `docs/orchestration.md` /
   `docs/architecture-reference.md` wherever they state the approval rule.
   Leave released notes alone.

## Delete — do not maintain

- `participant_or` in `ops/task/ship.rs`: `process_starter` replaces it.
- The `ships` → unconditional `ship_in` path in `start_run_in`, and the
  unconditional `ship_in` calls in `arrive_in`/`end_task_graph`. They are
  reshaped into `Approval::Route`, not kept beside it.
- The `Person { name }` steer branch for agent-provenance comments in
  `append_task_comment`.

## Forbidden outcomes

- A second approval column or grant (the #1736 shape). The stop only blocks
  implicit approval.
- Starter names persisted as text: always derive them from Process ancestry.
- A run-start refusal instead of the landing boundary. Relaunches must keep
  working.
- `#[serde(default)]` on any new DTO field.

## Absent / error states

- No participant name: the starter renders `attribution unresolved`, never "someone".
- A Process row missing from the chain (pruned history): `Starter::Loopflow { command: None }`
  for a non-root, and the person for a root.
- Approval read fails at a landing step: the step fails (no merge). Unknown is
  never read as approved.

## Exclusions

- `lf task interrupt` disarming auto-merge.
- The `deliver`-at-end recovery that #1741 removed.
- Rewriting the historical comments already attributed to Jack.

## Acceptance at gate

`cargo test -p loopflow --lib task_graph_launch_tests::task_ship_stop` →
passes, including the new
`task_ship_stop_sticks_through_a_headless_relaunch`: ship, stop, then
`lf task run --from <first step>` on an all-headless graph ending in
`cmd: pr land` (fake gh). Expected: state ≠ shipping, the graph is
`At { step: "pr land" }`, fake gh saw no merge/auto-merge, the status JSON has
`ship_stop.by` = the participant, and `started_by` is the person. A
caller-session env variant shows `Session`. Then `lf ship` reaches `end`.
Also run `cargo test -p loopflow --lib store::sqlite::task_work::` (Route
approval is skipped under a stop; Explicit clears it) and the Swift
`TaskComments` / DTO fixture tests through `scripts/test.py`.

## Implemented (2026-10-29)

- Jack's steer, first slice: task_operate, task_session, launch-plan,
  repo/wave session, wave_operate, the `task_graph.rs` guidance hint and
  `lf task run`/`lf ship` help say `lf task run` launches and `-b`/`lf ship`
  approve Auto-Ship, only on an explicit ask to ship or land. Splitting `-b`
  from approval in code stays a separate decision.
- Migration `ship_stop.sql`: `ship_approved_lf_process_id`, `ship_stopped_at`,
  `ship_stopped_lf_process_id` (plain columns, no FK; prune keeps the rows).
- `ship_in(.., Approval::{Explicit, Route})`; `stop_shipping(.., stopped_by)`
  records a stop even when not shipping; `ship_halt` records none.
- Landing boundary: `task_graph::landing_name` (Clap `Cli::merges`: `pr land`,
  `pr arm`, any spelling; skill `pr-land`). A Task run reaching one while a stop
  stands returns `SkillOutcome::Unapproved` → arrives at that step;
  `select_task_run` starts at a landing step it waits at.
- `Starter { Person | Session }` from `SqliteStore::process_starter`;
  `TaskSnapshot.started_by/ship_approval/ship_stop` (`ShipMark { at, by }`),
  printed by text `lf task status`.
- `TaskCommentAuthor::Session { session_id, requester }` for agent steers,
  `<!-- loopflow-session:… -->` round-trips through Linear; steer text
  `Comment X by Session <id> for Jack Heart`. Swift model/view and DTO fixtures.
- Deleted: `participant_or`; unconditional route `ship_in` calls; the
  `lf task comment` list's "unnamed person" label (it prints the author's
  Display, "attribution unresolved").

## Remaining

- Desktop TaskWorkView: show `ship_stop` where the shipping reason goes and
  `started_by` on the run row (Swift `Starter`/`ShipMark` decoding, fixtures).
- Swift build/tests of the comment author change run at gate (`scripts/test.py`).
- Demo on a real Task (see Demo).

## Done when

After a person stops shipping, no relaunch by them, by a Task Session or by
recovery makes the Task shipping or merges its PR until someone runs `lf ship`.
Status and Desktop name who approved, who stopped, and who started each run.
Agent-written reasons show the Session as author and Jack as the requester.

Check: `cargo test -p loopflow --lib task_graph_launch_tests::task_ship_stop` (3) + `store::sqlite::task_work:: ops::task::ship::` + `task_graph::tests::` + `--test integration dto_fixtures::` + clippy → pass; Swift deferred to gate.
