# LOO-561 · Ship it — plan

Status (2026-10-22, fifth concept review): design accepted (Jack Heart, 2026-10-21/22); every accepted
slice is built and committed, vocabulary drift included. What's left: re-demo and gate (see Remaining). Post-merge verify is
on hold. This Task's schema draft is `store/migrations/drafts/ship_it.sql`, the only draft on the branch.

## Direction from Jack Heart (2026-10-21, design review)

Verbatim:

> - it should basically proceed from whereve you are simulating the interactive steps
> - i think we want workflows to just be graphs with -i on the ones that are meant to be interactive basically
> - the task session stays open but is no longer default shown in the working set

> ship it doesnt skip demo, it just demos non-interactively

> we need another word for what is currently the ship flow now

> i do think it would be nice to also lf ship <flow that defaults to today's ship> even from a
> non task and have that maybe launch a background task with that flow

> it shouldnt ever need me
> ship it.

> theres no channel to ask for help. figure it out and move forward.

> i want to make sure background tasks are a good UX but its ok to require & or nohup or wahtever

## Accepted decisions

- **Names** (Jack): the old `ship` graph is **`deliver`** (`gate → land --wait-and-fix`);
  builtin `deploy` is deleted. Step syntax `interactive: <skill>` ("yeah that works").
- **One definition kind** (Jack): a Workflow is a graph; `interactive:` steps are stages, routes
  are the slices between them. `interactive:` inside `xor` is allowed ("drop the rejection").
- **`lf ship [GRAPH]`** runs in the foreground until merge and completion; the caller backgrounds
  it. Outside a Task it creates one for the branch (default branch refused). `lf ship <graph>`
  takes the graph up like `lf task run X <graph>`. Idempotent on a live carrier. Stand-ins
  decide at every stage; nothing asks Jack. No NeedsYou state.
- **Lifecycle** (Jack, 2026-10-22: "it should be live state", "Working", "Shipping for, now"):
  `unstarted → working → shipping → done`, plus `abandoned`; `paused` is a hold. A Task becomes
  `shipping` on any Task-API route into the stage-free tail of its graph (Ship it, choosing
  `deliver`, an `autonomous` run); a direct `lf land` does not. "auto tasks become auto shipping".
- **Stop shipping** (Jack: "yes, disarm"): shipping → working, ends the carrier, disarms the
  auto-merge Loopflow armed, recorded in history.
- **Workflow travels** (Jack: "yes, workflow should travel"): captured graph and stage sync
  with the shared Work model. Machine-local execution (carrier, liveness, attention) stays local;
  only the checkout's Machine restarts a carrier.
- **One landing method, lazy recovery** (Jack: "that works for me"; "there hsouldnt be two
  methods between ci watch and land and fix"; recovery "universal and not turno ff able"): the
  ship process is the only landing engine and the only repair starter. Any lf read of this
  Machine's Tasks/processes settles dead AgentProcesses and restarts a shipping Task's dead
  carrier. No schedule, no setting. Accepted gap: a Machine nothing reads recovers on next read.
- **Desktop views** (Jack: "default should filter them out"): build on #1580; the Working query
  excludes `state:shipping`; Shipping is a `state:shipping` query, not a `SidebarFilter` case.

## Landing evidence (Jack's infrastructure pass, 2026-10-21, comment 6e0e4ba4)

Five hand restarts of `lf pr land --wait-and-fix` in one day. Each is handled on the branch:

- **Arm race** (#1624, #1639, #1613, #1628): reasonless drop re-read after 5 s, never exits.
- **Queue drop, auto-merge on** (#1613): re-entry via `enqueuePullRequest`, else exact-head re-arm.
- **Late failures** (#1624, #1638): `RequiredProducers` from `.github/workflows` promotes a failed
  leaf in the required check's `needs` closure while `tests-result` is absent.
- **Flakes dominate**: `failed_checks` re-queues once before ci-fix; no early queue-commit
  repair (settled 2026-10-22). LOO-562 owns the Swift hang.
- **Completion on any merge** (#1619, #1614): the ship process completes a shipping Task once
  its PR merges, whoever merged it (6a337048f7). Hosted proof is left to demo.

## Built (code is the reference)

- Graph model: `interactive:` step kind (top level and loop decider), stage id = skill name,
  `human:` and `require_autonomous_steps` deleted, `.lf/workflows/` gone, builtins `feature`,
  `code`, `research`, `deliver` under `builtins/task/graph/`. `InteractionGraph` projects stages
  and routes (edge `name`, not `flow`); `lf task run ISSUE [ROUTE]` runs one slice to the next
  stage. Migration keeps `end` rows, drops the rest (re-placed by stage id), rewrites
  `design`→`review-design`, `findings`→`research` in history.
- Landing (`ops/pr.rs`, `pr_landing.rs`, `ci_watch.rs`): `Dequeued{head_sha, reason}` from
  timeline events; re-arm/re-queue with Task `Progress` events; attempts `<identity>#n` with
  1→30 min backoff and prior summaries; `wait_for_landing` returns only on merge or close; Pause
  holds re-arm; "required check producer unknown" shown; no elapsed-time cutoff.
- Ship (`ops/task/ship.rs`, `lf/commands/ship.rs`): stand-in line at each stage, advance/repeat
  through the decision output, `blocked` repeats with the blocker; taskless branch Task;
  Pause/Stop rechecked between steps and after each stand-in.
- State: `TaskState` is the lifecycle (Rust, Swift, fixtures). Stored `tasks.shipped_at` (shared
  WorkValue `task_shipped_at`) and local `tasks.ship_lf_process_id`; reopen and Stop clear both
  through `task_work::unship_in`. `ShippingSnapshot{shipped_at, state, reason}` with
  `Building|Landing|Repairing|Restarting|Paused` and `ship_control` on `TaskDetailSnapshot`/
  `RoadmapTask`. `Shipped` move kind deleted. `flow_finished` events read as `GraphFinished`.
- Desktop: `TaskShipControl` on the main-pane Task heading (`TaskWorkView.swift`) holds **Ship
  it** (`task-ship`, the only route into `end`) and, while shipping, `state · reason` with **Stop
  shipping**. `ShipNoticeToast` Undo = Stop. `RegistryQuery.shipTask` runs `-b --task X ship`
  detached. Sidebar rows and `TaskActionItems` menus carry no ship control, and there's no
  `SidebarFilter.shipping` (Shipping is the #1580 `state:shipping` query).
- One engine (2026-10-22): `lf ci watch`, `CIWatchers.swift`, `lf task reconcile/automation/
  automate`, the repository cron (`lf cron sync` removes released copies) and
  `tasks.automation_enabled` deleted; Pause holds repair and re-arm; `ops/recovery.rs` runs on
  `task status/list`, `wave status/list/show`, `history`, `monitor`.
- `lf ship` completes on merge (`ship_complete_if_merged`), backs off blocked stand-ins
  (`blocked_backoff`, 1→30 min, rechecking Pause/Stop).
- `ops/task_automation.rs` and `store/sqlite/automation.rs` deleted (63f13b3805): execution blockers live in `ops/task_execution.rs`, repair admission
  (`RepairAttempt`, `RepairReservation`) in `store/sqlite/ci_incidents.rs`, `repository_tasks`
  in `pr_landing.rs`; the stand-in decision in `lf/commands/ship.rs` reads as one match.
- Desktop Customize for Graphs and Workflows is one `WorkModel.definitionFile` over `lf graph customize`
  (`.lf/graphs/`); Swift `workflowSource` deleted. `two_people` and `WorkCLISubscriptionTests` fixtures
  author graphs with `interactive:` stages. Check: `cargo test --test two_people` ok; `swift test --filter
  'customizedWorkflow|taskLifecycleUpdatesOpenSidebar'` ok.

## Remaining

1. **Re-demo** on the branch build. The pending items are under Demo findings: the heading
   placement (variant B), the `loopflow://` world links and `unstarted` world Tasks.
2. **Not in this Task:** the `automation:` config key now names a deleted concept (its only field
   is repair startup retries). `.lf/config.yaml` is read by released lf, so a rename needs the
   shared-file fallback plus a removal Task; leave it. (Both 2026-10-22 concept reviews' docs drift is fixed: docs teach `interactive:` stages, routes,
   `.lf/graphs/` and the repair backoff; run-time text and the work, launch-plan and
   wave-operate skills say route and stage. Loop backward edges and wire `edge` fields stay.)
3. **Gate.** Run the acceptance check below. Also clear the shared-runner isolation failures:
   `planning_lookup_tests` and some planning/store tests fail only when they share a runner with
   `ship`/`task_work`/`ops::linear` and pass alone. The pre-existing
   `agent_session_identity_is_durable…` failures are still open.
   Check (2026-10-22, on 71dd11e3c8, rerun on 042a007194): both flagged launch tests pass alone; the chooser refuses a
   bare run at `review` naming both routes, and a failed completion at `end` keeps the arrival,
   leaves completion pending and never replays (main's
   `successful_final_graph_keeps_arrival_when_completion_fails_and_never_replays`).
   `cargo test --lib -- workflow graph:: ship` 192/194, only the two `planning_lookup` isolation
   failures (22/22 alone); clippy ok. Deleted
   `a_task_takes_up_the_workflow_when_an_autonomous_graph_has_the_same_name`:
   with one definition kind in `.lf/graphs/` it overwrote the Workflow it meant to shadow.
   Settled (fifth concept review, 2026-10-22): a failed completion at `end` exits non-zero
   naming the unmerged PR; arrival and pending completion stay, nothing replays. The accepted
   contract already implies it: reaching `end` *requests* completion and a failed request
   retries, so the run reports a real failure rather than success. `lf ship` never meets this
   case because `deliver` lands before `end`. No code change.

Settled (2026-10-22):
- State-model cleanup (was item 3). `AbandonIntent`/`abandon_requested_at` and the duplicate DTO
  `completed` bool are deleted (Rust, Swift, fixtures); `state == "completed"` says it.
  `tasks.planning_completed(_at)`/`planning_state` stay: they are the done authority in the portable
  `Disposition` WorkValue that released peers read. `completion_request/error` stays as the retry
  path for a failed completion. Folding either is a shared-wire change with its own Task.
  Backlog/Todo↔unstarted, In Progress↔working, Done↔done and Canceled↔abandoned go through
  `task_state_delivery::queue_in`. In Review↔shipping is out of this Task: In Review is a named
  `started` state and needs a per-team lookup and a rule for inbound edits.
- Completion on merge is the ship process's job: before each route it reads the PR and completes
  the Task once merged. A non-shipping Task still follows "merge leaves the Task open until
  completion"; `lf pr reconcile`, `lf land` and reads settle it.
- Placement (Jack, variant B): main-pane heading only, hover-revealed. Variant C (Workflow rail)
  comes later and is still unsettled.

4. **Post-merge `verify` (Jack Heart, accepted 2026-10-23: "yea sound good"). Not built yet.**
   - Stage: `interactive: verify` after `deliver` (`… → deliver → interactive: verify → end`).
     Jack: "no verify is not a stage only humans can do". A person usually, a stand-in when
     shipping. Whether a Task has anything to verify is per Task ("its very task by task if
     needed"); whoever is at the stage says so and moves on.
   - Skill: Jack: "over time i hope to make robots do it almost exclusively / but rigt now they
     often think they cant". So `verify` teaches the headless routes (branch/main `lf` with its
     own `LF_HOME`, lf-world fake worlds, `loopflow-dev.py run-debug --world`, app builds and
     view/interaction tests, headless Chrome screenshots, the installed binary after release),
     and "can't verify X" is a finding: build the missing route in the fix, or file it as a
     follow-up Task. Never a hand-back.
   - Outcome: Jack: "Verify should either close the task OR start a new PR to fix whats found."
     Pass → `end`, the Task completes. Findings the Task promised → fix forward in the same Task:
     branch restarts from main, a new current PR, the earlier one kept as delivery history (the
     existing serial-PR rule), `deliver` again. Findings outside its outcome → linked follow-up
     Task. No revert. Jack on multi-PR vs follow-up Tasks: "allowing multiple PRs on task may be
     messy but may also adapt better to arbitrary stuff happening" — revisit after real use.
   - Operator settlements, reversible: a merge while shipping moves the Task to `verify` instead
     of completing it (hand-merged PRs still get verified); the auto-shipping rule stays measured
     to `end`, so a Task with `verify` ahead is not auto-shipping.

## Done when

- Ship it or `lf ship` on a `feature` Task at `review-design` runs to merge and completion with
  no person, headless `demo` included; history shows who shipped it and each stand-in decision.
- A shipping Task appears under the `state:shipping` query, not Working; its conversation stays open.
- `lf ship` on a bare branch creates a Task that Shipping shows.
- A second `lf ship` exits 0 "already shipping"; a killed carrier restarts on the next lf read;
  Pause stops it, resume restarts it; Stop returns it to working and disarms.
- A failed leaf in `tests-result`'s `needs` closure starts a repair within one poll.
- #1613/#1628 patterns (queue removal for `failed_checks`/`checks_timed_out`, manual disable)
  merge with no hand.
- One definition kind; `lf ci watch`, the periodic check and `automation_enabled` are gone.

## Risks

- Endless repair or stand-in loops on chronic flakes: evidence-driven with backoff, attempt count
  visible, Pause stops. Named Wave risk (agent spend).
- A stand-in demo is weaker than Jack's: accepted by direction; decisions stay in history.
- Lazy recovery leaves a sleeping Machine's carriers dead until something reads lf (accepted).
- Unknown required-check producer: the landing waits for the required check.

## Acceptance check at gate (headless)

```bash
cargo test -p loopflow --lib -- workflow graph:: pr_landing ops::pr::tests ci_watch ship
cargo test -p loopflow --test integration -- task_workflow task_graph_launch dto_fixtures
swift test --package-path swift --filter 'DTOFixture|WorkModelTests|WorkNavigation|DesktopHeadless'
```

The hosted merge-queue run is left to demo.

## Demo findings (2026-10-22, branch build on lf-world `portfolio`)

- Jack accepted the naming ("ok that works").
- Header not visible on main or branch: Ship it now lives on the main-pane heading
  (`TaskShipControl`, 3f906af16f); re-demo pending.
- `loopflow://` links in the dev app resolve against the real `~/.lf`, not the world's
  `LF_HOME`. Not checked against main.
- World Tasks that ran a Graph read `unstarted` on the branch. Not a state regression: main's
  `task_state_sql` also derived from Workflow position (`ready` at `start`, `active` past it);
  the branch renamed those to `unstarted`/`working`. A Graph run taskless in a Task's checkout
  never moved the Workflow on either. Kept position-derived; whether checkout work alone should
  read `working` is an open question for Jack at re-demo (scratch/questions.md).

Check (2026-10-22): clippy/fmt clean; `dto_fixtures` 42 ok; lib pr_landing/chapter/children/waves/
task_execution ok; mixed-runner planning/store failures pass alone (owner: gate).

## Re-demo (2026-10-23, branch build as separate "Loopflow 561" app on a fresh lf-world `portfolio`)

- Fixed since the last demo: a Task waiting at `demo` reads `working` (was `unstarted`).
- **Ship it not visible** (Jack: "i dont see the ship it button", on fd380b at `demo`). Cause:
  `TaskShipControl` is mounted only in `WorkSurfaceView`'s Task detail page. A started Task
  (has an owner) opens to `WorkspacePanesView` (Session + Files panes, `SessionsView.swift`
  `workspacePanel`), which has no Task heading, so every Task that can ship hides the control.
  The "main pane" Jack chose has to be the Session workspace, not the detail page.
- Not this Task: "Layout not saved: error: root --json requires --context or --explain" banner.
  Desktop saves pane layout with `lf desktop layout <work> --save --json`; `desktop layout` has
  no own `--json`, so the root guard rejects it. Same code on origin/main.
- **Placement corrected** (Jack, 2026-10-23: "I think near where it sys Started though"). Ship it
  goes in the window's jump bar, beside the Task's state in `TaskHeading` (`WorkNavigator.swift`,
  the breadcrumb that reads "LOO-… title Started", before the Machine control). It shows for
  every selected Task, whatever pane layout is open. Hovering the heading fades in the green
  Ship it next to the state; while shipping the same spot holds Stop shipping, and the Undo
  toast stays. Delete `TaskShipControl` from `WorkSurfaceView`'s detail page (one place only).
  That state text still says "Started"/"Unstarted" from the old model; it reads the lifecycle
  instead: Unstarted, Working, Shipping · reason, Paused, Done, Abandoned.
- Not this Task: the jump bar's Machine control shows "Swift.CancellationError" with a ⚠
  (`WorkMachineControl.load()` reports the cancelled `.task(id:)` load). Fixed in this Task at Jack's request (6a066d80a4: `CancellationError` is ignored).

