# LOO-561 · Ship it — plan

Status (2026-10-22, implement pass): design accepted (Jack Heart, 2026-10-21/22). Built: graphs
with `interactive:` stages, the landing engine fixes, `lf ship` with stand-ins, Shipping lifecycle
with Stop, #1580 taken (Shipping is `state:shipping`), one landing engine with lazy recovery,
completion on merge while shipping, blocked backoff, Ship it on the Task heading. Remaining: the
rest of the state-model cleanup (item 3); commit the uncommitted `task_automation` fold. Drafts beside this Task's `ship_it.sql`: #1643's
`rename_flow_to_graph.sql`, main's `detached_owner_viewers.sql`, `saved_selections.sql`,
`two_person_gaps.sql`.

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
- **Completion on any merge** (#1619, #1614): unverified on the branch (see Remaining).

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
- Desktop: `TaskWorkflowHeader` mounted in `WorkSurfaceView` with **Ship it** (`task-ship`,
  replacing route buttons into `end`) and **Stop shipping**; `TaskActionItems` both;
  `RegistryQuery.shipTask` (`-b --task X ship`, detached); interim `SidebarFilter.shipping`;
  rows show `state · reason`.
- One engine (2026-10-22): `lf ci watch`, `CIWatchers.swift`, `lf task reconcile/automation/
  automate`, the repository cron (`lf cron sync` removes released copies) and
  `tasks.automation_enabled` deleted; Pause holds repair and re-arm; `ops/recovery.rs` runs on
  `task status/list`, `wave status/list/show`, `history`, `monitor`.
- `lf ship` completes on merge (`ship_complete_if_merged`), backs off blocked stand-ins
  (`blocked_backoff`, 1→30 min, rechecking Pause/Stop).
- `ops/task_automation.rs` and `store/sqlite/automation.rs` deleted (uncommitted at realign,
  2026-10-22): execution blockers live in `ops/task_execution.rs`, repair admission
  (`RepairAttempt`, `RepairReservation`) in `store/sqlite/ci_incidents.rs`, `repository_tasks`
  in `pr_landing.rs`; the stand-in decision in `lf/commands/ship.rs` reads as one match.

## Remaining

3. **State-model cleanup.** Done: `AbandonIntent` and `abandon_requested_at` (Task and Project)
   deleted; nothing set them, and the Task save nulled `abandon_reason` on every write. Left:
   `tasks.planning_state`/`planning_completed(_at)` (Linear's mirrored state; read by
   `linear/acceptance.rs` writeback and `ops/task.rs`; distinct from the `PlanningState` evidence
   enum, which stays), duplicate DTO state fields, and folding `completion_request/error` into the
   done transition plus writeback if nothing else reads it. Linear mapping:
   Backlog/Todo↔unstarted, In Progress↔working, In Review↔shipping, Done↔done,
   Canceled↔abandoned, through the one writer.

Settled this pass (2026-10-22):
- Completion on merge is the ship process's job: before each route it reads the PR and completes
  the Task once merged. A non-shipping Task still follows "merge leaves the Task open until
  completion"; `lf pr reconcile`, `lf land` and reads settle it as before.
- Placement (Jack, variant B): Ship it/Stop shipping in `TaskShipControl` on the main-pane
  heading, hover-revealed; `ShipNoticeToast` Undo = Stop shipping. Not in `TaskWorkflowHeader`,
  sidebar rows, the sidebar/Dashboard/Sessions menus (`TaskActionItems`). Variant C (Workflow
  rail) is later and unsettled.

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
- Header not visible on main or branch: fixed by mounting it (ec9bdfdcfe); re-demo pending.
- `loopflow://` links in the dev app resolve against the real `~/.lf`, not the world's
  `LF_HOME`. Not checked against main.
- World Tasks that ran a Graph read `unstarted` on the branch; main's builder says a Graph in a
  checkout starts its Task. Possible regression, unverified.

Check (2026-10-22 realign): clippy clean; `cargo test --lib -- pr_landing ship task_execution
task_work` 103 pass, 3 `planning_lookup_tests` fail only under that mix (22/22 pass alone) — gate.
Check (2026-10-22 second implement pass): lib cron/pr_landing/ship/doctor/task_work/migrations/
canonical_ids/planning, task_graph_launch ship tests (5), integration documented_commands/
cli_reference/dto_fixtures, Swift DesktopHeadless/DTOFixture/WorkModel/WorkNavigation 137 pass,
clippy clean. Earlier pre-existing failures (`agent_session_identity_is_durable…` etc.) deferred to gate.
