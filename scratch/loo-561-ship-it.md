# LOO-561 · Ship it — plan

Status (2026-10-24): design accepted (Jack Heart, 2026-10-21/22); every accepted slice is built
and committed, including the 2026-10-24 pass (Workflow bar, ship-restart backoff, post-merge
verify) and the second pass (Task/Session state axes, `lf ship ISSUE`). What's left: re-demo
on the branch build, and gate (see Remaining).
Main merged at 7f63da1649 (2026-10-24). This Task's schema draft is
`store/migrations/drafts/ship_it.sql`; main's unreleased `pools_and_rentals.sql` (#1653) sits
beside it and touches none of the same tables (main's #1655 rewrite released in `0.14.15.002`).

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
- **Placement** (Jack, 2026-10-24: "dots with some progressive disclosure or click to expand"):
  a Workflow bar under the jump bar holds the stages and Ship it / Stop shipping, always
  visible. Supersedes the hover control in the jump-bar heading (2026-10-23) and "variant C later".
- **Post-merge verify** (Jack, 2026-10-23: "yea sound good"; pass approved 2026-10-24): see
  Remaining 4. The replayed 2026-10-22 hold steer (4a36e3f0) is superseded.

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
  `RoadmapTask`. `Shipped` move kind deleted. Released `0.14.15.002` (#1655) rewrites `task_events`
  `flow_finished` rows; the branch's serde alias stays for Project `TaskObserved` copies.
- Desktop: `TaskShipControl` in the Workflow bar (`TaskWorkflowBar`, `TaskWorkView.swift`) holds **Ship
  it** (`task-ship`, the only route into `end`) and, while shipping, `state · reason` with **Stop
  shipping**. `ShipNoticeToast` Undo = Stop. `RegistryQuery.shipTask` runs `-b --task X ship`
  detached. Sidebar rows and `TaskActionItems` menus carry no ship control, and there's no
  `SidebarFilter.shipping` (Shipping is the #1580 `state:shipping` query).
- One engine (2026-10-22): `lf ci watch`, `CIWatchers.swift`, `lf task reconcile/automation/
  automate`, the repository cron (`lf cron sync` removes released copies) and
  `tasks.automation_enabled` deleted; Pause holds repair and re-arm; `ops/recovery.rs` runs on
  `task status/list`, `wave status/list/show`, `history`, `monitor`.
- `lf ship` settles a merge (`ship_settle_merge`: on to verify, else complete), backs off blocked stand-ins
  (`blocked_backoff`, 1→30 min, rechecking Pause/Stop).
- `ops/task_automation.rs` and `store/sqlite/automation.rs` deleted (63f13b3805): execution blockers live in `ops/task_execution.rs`, repair admission
  (`RepairAttempt`, `RepairReservation`) in `store/sqlite/ci_incidents.rs`, `repository_tasks`
  in `pr_landing.rs`; the stand-in decision in `lf/commands/ship.rs` reads as one match.
- Desktop Customize for Graphs and Workflows is one `WorkModel.definitionFile` over `lf graph customize`
  (`.lf/graphs/`); Swift `workflowSource` deleted. `two_people` and `WorkCLISubscriptionTests` fixtures
  author graphs with `interactive:` stages. Check: `cargo test --test two_people` ok; `swift test --filter
  'customizedWorkflow|taskLifecycleUpdatesOpenSidebar'` ok.

## Remaining

**Built 2026-10-24, second pass (Jack approved: "yep, run pursue"):**
- d. Two axes. `TaskState` is unstarted · idle · active · moving · shipping · paused · done ·
  abandoned (`moving` per Jack: "yeah, i think im leaning towards moving"), derived in
  `task_state_sql`: a Graph runs while the route carrier (`task_workflows.lf_process_id` on an
  edge) or the ship process has no `completed_at`; the Ship it approval (`shipped_at`) makes
  that run shipping; approved with nothing running reads idle (the Workflow bar keeps the
  failure, next restart and Stop shipping); at a stage reads active; no checkout reads
  unstarted. The ship engine reads its approval, not the state. Working tasks (repository
  default) = idle, active, moving (Jack: "active, moving, idle all count"); paused and
  unstarted left it. Query terms: Task `unstarted|idle|active|moving|shipping|paused`, Session
  `available|busy|blocked` (`running` → `busy`; Session `idle` gone); Desktop's "Running"
  filter is "Busy". Linear mapping unchanged (event-driven: idle/active/moving are In Progress).
  Chapter "begun" keeps its Workflow meaning. Session states are only renamed in queries and
  the sidebar filter; the Session's own attention code is untouched.
- e. `lf ship [ISSUE] [--graph GRAPH]`; `Cli::checked` lifts ISSUE into `--task`.
- Check: `cargo test --lib` selections/selection_tests/task_work/ship/chapter 38+40 ok;
  `dto_fixtures`/`documented_commands` 46 ok; CLI reference regenerated; Swift
  `DTOFixture|WorkModelTests|WorkNavigation|DesktopHeadless|TaskHistoryFilter` 147 ok; clippy
  and fmt clean. Broad lib run under load: only planning 5 s deadline timeouts and
  `os_process::private_home_keeps_explicit_binary_selection` (pre-existing; owner: gate).
- Gap (scratch/questions.md): a route carrier killed without settling reads moving until settled.
- Compress (2026-10-24): selections map `TaskState` to one `SelectionState` (done/abandoned → `done`);
  chapter "begun" reads Workflow past `start` or the approval, not a state list. `cargo test --lib
  selections selection_tests chapter task_work` 63/64, the one a known shared-runner planning
  isolation failure that passes alone (owner: gate); clippy/fmt clean.

**Built 2026-10-24 (Jack approved the pass: "yes"), from Re-demo 2 below:**
- a. `TaskWorkflowBar` (`TaskWorkView.swift`) under the jump bar on `RepoView`: Workflow name,
  stages + `end` as dots (hover names, click opens stage cells: who, and arrived/here/ahead), the
  live line (shipping reason, running route + Graph step, "At <stage> · yours" — seventh
  concept review: "try" was demo's word), and
  `TaskShipControl` always visible. `ShippableTaskHeading` deleted; the jump-bar heading reads
  only the lifecycle ("Shipping"), so shipping progress no longer rewrites it. Not built: the
  live line's elapsed time and Session link from the prototype.
- b. `tasks.ship_restarts` (in the draft): lazy recovery restarts a *failed* ship process no
  sooner than `repair_backoff(restarts+1)` after it ended; a completed step resets the count;
  a killed carrier restarts at once. The reason reads the deepest failed child's error
  (`SqliteStore::failure_cause`): "Ship failed: agent exited with code 1…; retrying in N min".
- c. Post-merge verify: `feature`/`code` end `deliver → loop: implement / interactive: verify →
  end`; builtin `verify` skill. `ship_settle_merge` sends a merged shipping Task to the stage
  whose route into `end` runs nothing, once per graph (no prior move into it); with none it
  completes. Stand-in repeat is any route back to the same or an earlier stage, so verify's
  repeat is the fix-forward route (`pursue` → demo → deliver → verify).
- Check: `cargo test --lib task_ship_` 7/7 and `ops::task::ship` 4/4; `--lib -- workflow graph::
  builtins task_workflow` ok but `linear_on_demand_graph_steps_acquire_before_spawning` (5 s
  planning deadline under load); `swift test --filter 'DesktopHeadless|WorkNavigationTests'` 52 ok;
  clippy/fmt clean.
- Test repair (2026-10-24): graph help compiles the graph again, so a duplicate stage id fails
  "not unique" (the `human_occurrence_ids` removal had dropped validation). Land tests no longer
  drive `ci watch`: the non-Task check case keeps only `pr reconcile` observing merge (its repair
  variants tested the deleted watcher repair); `waited_task_landing` is one waiter held by live
  work, read via `task status` (lazy recovery), then released. GitHub fixtures carry
  `timelineItems`. Check: `land_tests` 30/30, `graph_help_validates` ok.
- Compress (2026-10-24): `merge_target`'s `Option<Option<&str>>` became `reached`; the Workflow
  bar computes its stages once; the Workflow header's routes-not-into-`end` filter is one
  `routes(_:)`. `swift test --filter 'DesktopHeadless|WorkNavigationTests'` 53 ok. `cargo test --lib task_ship_` 7/7 (with `LOOPFLOW_TEST_LF` set:
  the support crate's compiled-in `CARGO` path is stale here), `ops::task::ship` 4/4, clippy clean, Mac build ok.

1. **Re-demo** on the branch build: the Workflow bar (dots, click-to-expand stage cells, live
   line, Ship it / Stop shipping), a failed stand-in that backs off and names its cause instead
   of the hot loop, the `loopflow://` world links and `unstarted` world Tasks. Open for Jack at
   re-demo (unresolved; none blocks implement or gate):
   - Settled (Jack, 2026-10-24: "yep. maybe optionally take a new graph to ship with"):
     `lf ship [ISSUE]` takes the Task like `lf task run`; the graph moves to an option,
     `lf ship LOO-1 --graph deliver` (operator's spelling, reversible). No issue: the checkout's
     Task, or a new one for a taskless branch, as today.
   - At `verify`, the session workspace's Workflow bar offers only Ship it, which runs a verify
     stand-in. A person who already checked main finishes through the conversation or the
     detail page's Complete. Does the bar want Complete at a post-merge stage?
   - Answered by the state redesign below (Jack, 2026-10-24).
2. **Not in this Task:** the `automation:` config key now names a deleted concept (its only field
   is repair startup retries). `.lf/config.yaml` is read by released lf, so a rename needs the
   shared-file fallback plus a removal Task; leave it. (Both 2026-10-22 concept reviews' docs drift is fixed: docs teach `interactive:` stages, routes,
   `.lf/graphs/` and the repair backoff; run-time text and the work, launch-plan and
   wave-operate skills say route and stage. Loop backward edges and wire `edge` fields stay.)
3. **Gate.** Run the acceptance check below (the branch's own integration failures are repaired; see
   Test repair). Also clear the shared-runner isolation failures:
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
- Sixth concept review (2026-10-23, jump-bar slice): nothing to change. The heading reads one
  lifecycle; a paused shipping Task reads "Shipping · Paused…" and keeps Stop shipping, which
  matches "paused is a hold". Open item stays Jack's: whether checkout work alone reads
  `working`. None blocks re-demo or gate.
- Eighth concept review (2026-10-24, after "At <stage> · yours" and `routes(_:)`): nothing to
  change. The header's routes exclude any route into `end`, which hides verify's pass route.
  That's the open Complete-at-verify question in Remaining 1, and it stays Jack's.

4. **Post-merge `verify`: built 2026-10-24 (see above); the shape below is the reference.** The hold steer of 2026-10-22
   was the operator's, posted under Jack's name while the shape was in discussion; Jack then
   accepted the shape on 2026-10-23 ("yea sound good"). Nothing is on hold.
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
cargo test -p loopflow --lib -- workflow graph:: pr_landing ops::pr::tests ship
cargo test -p loopflow --test integration -- task_workflow task_graph_launch dto_fixtures land_tests graph_help_validates
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
- **Placement corrected** (Jack, 2026-10-23: "I think near where it sys Started though"). Built:
  `ShippableTaskHeading` in the jump bar (`WorkBreadcrumbs` trail) holds `TaskHeading` plus
  `TaskShipControl`, fading in Ship it / Stop shipping on hover for every selected Task. The
  heading state reads the lifecycle (Unstarted, Working, Shipping · reason, Paused, Done,
  Abandoned). The toast is window-level (`WorkNavigation.shipNotice`, `ShipNoticeOverlay` on
  `RepoView`). The detail page's control is gone; `WorkModel.shipTask`/`stopShipping` post the toast, so
  the control and Undo carry no callback. Check: `swift test --filter
  'WorkNavigationTests|DesktopHeadless'` 51 ok (2026-10-23, after compress).
- Not this Task: the jump bar's Machine control shows "Swift.CancellationError" with a ⚠
  (`WorkMachineControl.load()` reports the cancelled `.task(id:)` load). Fixed in this Task at Jack's request (6a066d80a4: `CancellationError` is ignored).

## Re-demo 2 (2026-10-24, jump-bar build, "Loopflow 561" on a fresh lf-world `portfolio`)

- Jack found Ship it in the jump bar and shipped b17275 (at `start` of `code`). The heading then
  read "Shipping · Ship process exited; it restarts on the next read of this Machine's Tasks"
  (Jack's screenshot).
- **Restart hot loop (real bug).** Each stand-in `implement` failed at once: "no eligible managed
  claude account: a connected managed account is required by this execution boundary" (the world
  has no managed account). The ship process exited, lazy recovery restarted it on the next read,
  and the app reads constantly: ~150 stand-in runs in two minutes, about one a second, until the
  operator ran `lf --task b17275 ship --stop`. Expected: a restart after a failed ship process
  backs off like a repair (1→30 min), and the Task's reason names the real failure, not
  "exited; it restarts". Same failure class hit LOO-561's own pursue (Claude account selection
  errors), so on a real Machine an expired login would spin the same way.
- **Reason hides the cause.** "Ship process exited" says nothing about the missing account.
- **CLI form.** `lf ship <issue>` is not a form: the positional is the graph. The Task comes from
  `--task <issue>` or the checkout. The operator's demo instructions (`wlf ship d5580f`) were
  wrong. Open: should `lf ship` take an issue (e.g. `lf ship LOO-1`) the way `lf task run` does?
- Not exercised: shipping past the first stand-in (the fake world cannot run headless agents).
- **Hover reveal flashes** (Jack: "its not really working with the flashing in the heeader").
  Likely causes, unconfirmed: the hot loop rewrote the heading's "Shipping · reason" text about
  once a second, shifting layout under the pointer; and `ShippableTaskHeading` keeps `hovered` in
  `@State` inside the jump bar, which refreshes with every Work read.
- **Direction under discussion** (Jack: "i also feel like maybe we doo wnat a separate tool bar
  for the workflow underneath"): a Workflow bar below the jump bar, holding the stages, the
  Task's position and Ship it / Stop shipping, visible without hover. This revisits variant C
  (Workflow rail), which Jack earlier called "eventually". Prototypes in
  `scratch/workflow-bar.html`.
- **Workflow bar chosen** (Jack, 2026-10-24: "i think probably dots with some progressive
  disclosure or click to expand or somethign would work"). Prototype variant 3 in
  `scratch/workflow-bar.html`. A bar under the jump bar: the Workflow's name, its stages as
  dots (hover a dot for its name), the live line (the Graph step running now with its time and
  Session link, or "At demo · yours", or the shipping reason including the real failure),
  and Ship it / Stop shipping at the right end, always visible. Clicking the dots expands a row
  of stage cells: who does each stage (you / agents / stand-in), and what happened there or will.
  Operator reading, reversible: the jump bar returns to navigation (crumbs, lifecycle state,
  Machine); the hover Ship it in `ShippableTaskHeading` is deleted; expanded/collapsed is
  per-viewer UI state. This supersedes "Variant C later" for this Task.

## Task states, redrawn (Jack Heart, 2026-10-24) — under discussion, not yet direction

Jack: "i think we basically want states for:
- has a checkout but not currently running a workflow (idle)
- currently at an interactivestage of workflow (waiting? yours?)
- currently running an atuonomous graph with at least one interactive stage left before shipping (working)
- no more interactive stages left, running a flow that culminates in completion and landing (shipping)"

Operator reading: the open-Task states become derived from checkout, Workflow position and
whether a Graph is running, instead of stored `working`/`shipping`:
- unstarted: no checkout yet (kept; Jack's list starts at "has a checkout").
- idle: checkout, no Graph running, not at an interactive stage (e.g. a stopped route).
- at an interactive stage: name open (Jack: "waiting? yours?").
- working: a Graph running with an interactive stage still ahead before shipping.
- shipping: no interactive stage left, a Graph running toward landing and completion. Ship it
  makes this true by standing in at the remaining interactive stages.
- plus done, abandoned, and paused as a hold.
Settled (Jack Heart, 2026-10-24), superseding the operator's earlier Available/Active draft:
"Available = task-session is avaialble" and "Active = current state is between graphs at a
strucutred convo in the task session". So:
- **Active** is the Task state: the Task sits at an interactive stage (between Graphs, in a
  structured conversation in its Task Session).
- **Available** is not a Task state. It stays the Session attention state AGENTS.md already
  defines: the Task Session's turn has ended and it is ready for the person. An Active Task's
  Session is Available or mid-turn; that is shown on the Session, not as another Task state.
- **Busy** is the other Session state (Jack, 2026-10-24: "Busy = task session is not
  available"): the Task Session is mid-turn. Available and Busy describe the Session only.
- **Working** (Jack, 2026-10-24: "Working = task is currently running a grpah, regardless of
  session availability"): a Graph is running for the Task, whatever its Session is doing.
  Shipping stays the case where no interactive stage remains and that run ends in landing and
  completion.
Task states: unstarted · idle · active · working · shipping · paused · done · abandoned.
Session states: available · busy.
Settled (Jack, 2026-10-24, "so its Working vs Active and Available vs Busy as two different
axes"; on a dead ship process: "i would say idle"): Task and Session states are two axes. A Task
whose ship process died reads **idle**; shipping is derived from a running ship Graph, not a
stored intent. Jack then defined it: "shipping is: graph is running *and* wont stop for chat again (either
because no caht or marked for chat)". Operator reading ("marked for chat" taken as marked to
stand in, i.e. Ship it pressed; confirm): shipping = a Graph is running AND no interactive stage
ahead will stop it, because none remain or Ship it marked them for stand-ins. The Ship it mark is
the stored part (a per-Task "stand in at remaining stages"), cleared by Stop shipping, by
reaching `end`, and by moving the Task back to a stage. A marked Task with no Graph running
reads idle; lazy recovery restarts its ship process from the mark, with backoff, and the idle
reason names the failure and the next restart. This keeps "recovery is lazy and universal".
Jack, 2026-10-24: "so there is an additional box we need to check to maintain "explicitly
approved for fast mode" or whatever but i dont think we generally need to differentiate in the
lists or searches". So the mark is one stored approval on the Task (the branch's existing
`task_shipped_at` WorkValue serves; its name is open), shown in the Task's own detail and
history, while lists and `state:` queries show only `shipping`, never "approved shipping" vs
"auto shipping".

