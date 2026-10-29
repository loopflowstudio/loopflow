# LOO-687 · Stopping ship sticks

Status: accepted (Jack asked to launch, 2026-10-29); implemented except
Remaining. Assumptions made while implementing are in
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
- **A stopped Task's run never lands.** `land` is the real merge authority: it
  is a graph step, not `shipped_at`. The runner checks for a standing stop when
  it reaches a merging step (`lf land`, `lf pr arm`, skill `pr-land`), not at
  launch, so a stop made mid-run counts too. While a stop stands, the run stops
  there and the Task waits at that step, the way it waits at an interactive one.
  `lf ship` continues from it. The gate is the stop, not "unapproved": an
  unapproved `code` run never reaches `end` after demo, so it would park at
  `land` every time (assumed, reversible; see questions).
- `ship_halt` (Loopflow withdrawing after identical repeated failures) does not
  record a stop. Its cause was the failure, and a relaunch that fixes it may
  approve again by reaching `end`.

Rejected: **explicit-only approval** (delete ship-on-run entirely). It's
simpler, but it reverses #1741's accepted behavior, under which `auto` and
`-b task run` land without `lf ship`. It would also still need the landing check,
because the graph's `land` step merges whether or not the Task is shipping.

## Source of truth

`tasks` row: `shipped_at` (approval, existing), plus migration `ship_stop.sql`:
`ship_approved_lf_process_id`, `ship_stopped_at`, `ship_stopped_lf_process_id`
(plain columns, no FK; prune keeps the rows). Approval and stop are mutually
exclusive: `ship_in(.., Approval::Explicit)` clears the stop, and
`stop_shipping` sets it, even when the Task isn't shipping, while clearing
`shipped_at`. `Approval::Route` (`start_run_in`, `arrive_in`, `end_task_graph`)
writes nothing while a stop stands. The stop columns are local, not a WorkEdit:
a new `WorkValue` variant would change the peer edit format that released peers
parse, so a Machine move doesn't carry a stop. The move already clears
`shipped_at`.

Who started what is **derived** from `processes`, never stored as names:
`SqliteStore::process_starter -> Starter { Person { name } | Session { id, name, requester } }`.
Walk the chain self → `parent_lf_process_id`. The first process with a
`caller_session_id` gives the Session. Otherwise the starter is the person: the
participant, or `attribution unresolved` when there is no name or the chain is
broken. There is no Loopflow variant, because ancestry can't tell `restart_ships`
children from a person's chain. A restarted ship inherits the reading process's
starter.

## Implemented (2026-10-29)

- Jack's steer, first slice: task_operate, task_session, launch-plan,
  repo/wave session, wave_operate, the `task_graph.rs` guidance hint and
  `lf task run`/`lf ship` help say `lf task run` launches, and that `-b`/`lf ship`
  approve Auto-Ship, which runs only on an explicit ask to ship or land.
  Splitting `-b` from approval in code is a separate decision, left for later.
- Landing boundary: `task_graph::landing_name` (Clap `Cli::merges`, any
  spelling). A stopped Task's run returns `SkillOutcome::Unapproved` → arrives
  at the step and prints
  `Waiting at <step>: not approved to merge (<by> stopped shipping). lf ship lands it.`.
  `select_task_run` starts at a landing step the Task waits at.
- Status: `TaskSnapshot.started_by`, `ship_approval` and `ship_stop`
  (`ShipMark { at, by: Starter }`, explicit Optional DTO fields with fixtures).
  Text `lf task status` prints them, including
  `Not shipping: <by> stopped it <time> · lf ship <id> approves it again`. That
  line is derived. No Progress event is recorded per skipped approval.
  `ShippingSnapshot` is unchanged, which keeps the roadmap fixtures stable.
- Attribution: agent-provenance steers get
  `TaskCommentAuthor::Session { session_id, requester }`, and
  `<!-- loopflow-session:… -->` round-trips through Linear. Steer text reads
  `Comment X by Session <id> for Jack Heart`. The persisted `durable::Author`
  stays `Named(requester)`: adding a variant is a reader-compat change that
  belongs in its own Task. Swift model/view and DTO fixtures are updated.
- Desktop: Swift `TaskExecution` decodes `lf task status`'s `execution`
  (work + `started_by`, `ship_approval`, `ship_stop`, explicit Optionals) and
  replaces the `{work}`-only reading (`WorkModel.taskDiagnostics` →
  `taskExecution`). The Workflow bar's live line appends `approved by <by>` to
  the shipping reason, or leads with the `Not shipping: …` stop line. The
  newest GraphProcessLog row says `started by <starter>` (the overview lists
  Graphs oldest first, as Rust's latest does). `task_checkout.json` carries a
  Session starter and a person stop.
- Docs: AGENTS.md quick reference, `docs/orchestration.md`, `ship.rs` doc.
- Deleted: `participant_or`; the unconditional route `ship_in` calls; the
  `Person { name }` steer branch for agent comments; the comment list's
  "unnamed person" label.

## Remaining

- Demo on a real Task (see Demo).

## Forbidden outcomes

- A second approval column or grant (the #1736 shape). The stop only blocks
  implicit approval.
- Starter names persisted as text: always derive them from Process ancestry.
- A run-start refusal instead of the landing boundary. Relaunches must keep
  working.
- `#[serde(default)]` on any new DTO field.

## Absent / error states

- No participant name, or a Process row missing from the chain: the starter
  renders `attribution unresolved`, never "someone".
- The stop read fails at a landing step: the step fails (no merge). Unknown is
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
`ship_stop.by` = the participant, and `started_by` is the person. The
`Session` starter is covered by the unit test
`process_starter_names_the_nearest_session_else_the_root_person`, not by an
integration variant. Then `lf ship` reaches `end`.
Also run `cargo test -p loopflow --lib store::sqlite::task_work::` (Route
approval is skipped under a stop; Explicit clears it) and the Swift
`TaskComments` / DTO fixture tests through `scripts/test.py`.

## Done when

After a person stops shipping, no relaunch by them, by a Task Session or by
recovery makes the Task shipping or merges its PR until someone runs `lf ship`.
Status and Desktop name who approved, who stopped, and who started each run.
Agent-written reasons show the Session as author and Jack as the requester.

Check: implement pass 2 ran `scripts/test_desktop.sh --filter 'DesktopHeadlessTests|DTOFixtureTests|LocalWaveAgentLauncherTests'` + `--test integration dto_fixtures::` → pass.
Check: implement pass ran `task_graph_launch_tests::task_ship_stop` (3) + `store::sqlite::task_work:: ops::task::ship::` + `task_graph::tests::` + `--test integration dto_fixtures::` + clippy → pass; realign `cargo test -p loopflow --lib process_starter_names` → pass; 
Check: compress ran `cargo test -p loopflow --lib ship` → 75 pass, 2 `planning_lookup_tests` failed under load and pass alone (22/22); clippy → pass.
