# LOO-561 · Ship it — design (draft, for review)

Status: design accepted in review (2026-10-21, Jack Heart); slices 1–4 built; 2026-10-22
implement pass built the stored Shipping state, Stop shipping and the mounted Workflow header
(see "Implement pass 2026-10-22" below). #1643 (Flow → Graph) is merged in (53968945e9);
its draft `rename_flow_to_graph.sql` sits beside this Task's `ship_it.sql` (renamed from
`ship_moves.sql`) and main's `detached_owner_viewers.sql`.

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

Decisions (2026-10-21). Jack confirmed 1 and 2. The operator took 3–6 under the direction above.

1. Today's `ship` graph is renamed **`deliver`** (`gate → land --wait-and-fix`). The builtin
   `deploy` (`gate → pr land`) is deleted.
2. Workflows become graphs with `interactive:` steps, inside LOO-561 (one PR).
3. `lf ship [GRAPH]` runs in the foreground until merge and completion. The caller backgrounds
   it (`&`, `nohup`, or Desktop spawning it detached). Outside a Task, it first creates a Task
   for the checkout's branch, so the Shipping view tracks the run.
4. In a Task, bare `lf ship` continues the Task's graph from its position. `lf ship <graph>`
   takes that graph up, as `lf task run X <graph>` does, and ships it from its placement.
5. Shipped agents never ask. A hand-disabled auto-merge is re-armed, a dead ship process is
   restarted by the minute-cadence `lf task reconcile`, and `lf ship` is idempotent. Only
   Pause stops it.
6. There is no NeedsYou state. Shipping rows say what is happening; none waits on Jack.

## Landing evidence from Jack's infrastructure Wave pass (2026-10-21, comment 6e0e4ba4)

`lf pr land --wait-and-fix` needed a hand restart five times in one day. Ship it must remove each:

- **Arm race.** #1624, #1639, #1613, #1628 printed "Enabling auto-merge..." then exited "pull request
  has no auto-merge request; run lf arm to resume landing"; GitHub showed auto-merge on a moment
  later. The waiter must re-read after arming, never exit on a stale read.
- **Queue drop with auto-merge still on.** #1613's queue run failed on flakes; GitHub removed it
  (`removed_from_merge_queue`) but left auto-merge on, and it never re-entered. A person re-queued
  it with the `enqueuePullRequest` mutation. Re-entry must use that path, not only re-arm.
- **Late or missed failures** followed from the arm-race exits (#1624 `swift-test`, #1638 a
  `rust-test` shard).
- **Flakes dominate.** A failure that also fails on main, or is a known flake, is rerun or re-queued,
  not handed to ci-fix to "fix" the PR. (LOO-562 owns the Swift hang itself.)
- **Completion on any merge.** #1619 and #1614 merged while their Flows were paused or running. The
  Task completes when its PR merges, whoever merged it.

## Concept review (2026-10-22) — ship intent is live state

Jack Heart: "yeah no it should be live state, not somethign in the history log". Accepted:
shipping is live Task state, set by `lf ship`, cleared by Stop/complete/abandon, each logged in
history for audit. The `ship_intent` field drafted here was superseded by the lifecycle below.

## Task state model (2026-10-22) — supersedes `ship_intent` above

Jack Heart: "we can make state like Working / Human / Active / Developing ? Shipping Paused
Unstarted ...Abandoned?" … "we may need to clean up other asspects of task workflow state
especially now that we omved to supporting other providers, sharing work models in git" …
"Let's make sure to simplify and align all the model here."

Accepted (Jack): Shipping is a Task lifecycle state, not history and not a side field; LOO-561
simplifies and aligns the whole Task state model rather than adding Shipping beside it.

Today's facts this replaces: `planning_state`, `planning_completed(_at)`, `abandoned_at`,
`abandon_requested_at/reason` (no writer since the supervisors were deleted), `completion_request/
error`, derived `TaskState` (`not_ready`/`ready`/`active`/…), the DTO's `task.state` + `task.completed`
+ `execution.status`, and this branch's `Shipped`-move scan. "Done" has two sources today.

Proposed model (operator draft; items marked "open" need Jack):

- **Shared lifecycle**, one stored value with one writer, synced through Git or Linear:
  `unstarted → working → shipping → done`, plus `abandoned`. Reopen returns to `working`.
  Ship it: working → shipping. Stop shipping: shipping → working. Merge settles shipping → done.
  Name `working` matches the *Working tasks* sidebar (open: Jack floated Active/Developing).
  Name for `shipping` open: operator offered `cleared` ("cleared to land": approved, finishing on
  its own); button stays **Ship it**.
- **`paused` is a hold over the lifecycle** (already a `paused` column). Pause keeps
  working/shipping underneath; resume clears the hold only.
- **Provider state is mapped, not mirrored**: Linear Backlog/Todo ↔ unstarted, In Progress ↔
  working, In Review ↔ shipping, Done ↔ done, Canceled ↔ abandoned, through the same writer and
  its writeback retry. No separate `planning_state`/`planning_completed`.
- **Machine-local execution stays derived and unsynced**: processes, ship-process liveness,
  attention (Available). Only the Machine holding the checkout (`checkout_machine_id`) restarts a
  dead ship process; others just read `shipping`.
- **Delete**: `abandon_intent` and its columns/DTO field, `TaskState`'s Workflow-derived
  `not_ready`/`ready`, the `Shipped` move kind and history scan, duplicate DTO state fields. Fold
  `completion_request` into the done transition plus writeback state if nothing else needs it.
- **Accepted (Jack: "yes, workflow should travel")**: the Task's captured Workflow and its position
  (the stage) travel with the shared Work model, so another Machine can continue a Task at its stage.
- **Open**: stopping a ship after it armed auto-merge — operator lean: Stop disarms what the ship
  armed (stop means nothing merges), recorded in history.

### Repair and watching: one owner (2026-10-22)

Found: three drivers start ci-fix through `pr_landing::run_repair` — the `land --wait-and-fix`
waiter, `lf ci watch` (running on Jack's Machine, repairing PRs with no waiter), and the periodic
Task check's `reconcile_repository` (disabled here). All are gated by `tasks.automation_enabled AND
NOT paused`; `lf task interrupt` turns that flag off as a side effect.

Jack Heart: "land --wait-and-fix can basically use ci watch as subtask if we want. no strong opinion
as long as its reliable and fast to determie things".

Accepted (Jack): one repair owner; detection must be reliable and fast. Operator decisions:

- `land --wait-and-fix` (the end of `deliver`, run by the ship process) is the only thing that
  starts a repair, and only while the Task is in the handed-off lifecycle state and not paused.
- `lf ci watch`'s observation (poll, gate reading, early leaf failure) is the shared subroutine the
  waiter uses; `ci watch` itself stops starting repairs. It keeps one job besides observing:
  restarting a dead ship process on the Machine that holds the checkout.
- Delete the periodic Task check (Jack: "remove the periodic task check i think"): its schedule,
  `reconcile_repository` as a scheduled job, and the schedule line of `lf task automation`.
  Inventory its other duties first; anything still needed moves to `lf ci watch` or is deleted.
- Delete `automation_enabled` (column, `lf task automate`, the interrupt side effect, the
  per-Task lines of `lf task automation`). `paused` is the only hold.

Jack Heart on AgentProcess cleanup: "lazy cleanup can work, or a cron?" … "but should be universal
and not turno ff able". Jack: "there hsouldnt be two methods between ci watch and land and fix".

Accepted (Jack): recovery duties must be universal and impossible to switch off; one landing method.
Operator reading: a cron is an installed schedule and can be off (this Machine's periodic check is
off today), so recovery is lazy, on lf's own read paths. AgentProcess reaping only settles
bookkeeping from OS exit evidence (`settle_agent_process`; it kills nothing), so it runs wherever lf
reads processes. Open: whether the same lazy rule restarts a dead ship process (shape A: the ship
process is the only landing engine, `lf ci watch` deleted) or a per-Machine watcher owns landing
(shape B, a resident process).

Accepted (Jack: "that works for me", 2026-10-22) — shape A, one landing method:

- The ship process is the only landing engine: remaining steps, then `deliver`'s
  `land --wait-and-fix`, which observes, repairs, re-arms, re-queues, merges and completes.
- The waiter adopts `ci watch`'s cheap reading: REST with `If-None-Match` while checks run (a 304
  costs no rate limit), GraphQL only to confirm a change or read the merge queue. ~15 s cadence;
  back off while waiting on the queue; respect the returned rate-limit budget. The GitHub limit is
  per account (5,000/h GraphQL points, separate 5,000/h REST), shared by all Machines and repos.
- Lazy, universal recovery (no schedule, no setting): any lf read of the checkout Machine's Tasks
  or processes — CLI, `lf ps`, Desktop — settles dead AgentProcess records and restarts a handed-off
  Task whose ship process is dead. Merge completes the Task through the same path.
- Delete: `lf ci watch` (and `--install`, `--parent-pid`, launchd service), Desktop's
  `CIWatchers.swift`, the periodic Task check (`lf task reconcile` schedule) and `lf pr reconcile`'s
  repair path, `automation_enabled`. `lf ci` stays as read-only status.
- Known gap accepted: a Machine where nothing reads lf (asleep, closed) recovers on next read.

### Decisions closing the concept review (Jack Heart, 2026-10-22)

> 1. Working
> 2. Shipping for, now
> 3. yes, disarm
> 4. didnt mention this, but if something hits a stage in a workflow where its auto from here on
>    out, then it should be marked as shipping. for now only the task apis, not a direct land invocation

- Lifecycle names: `unstarted → working → shipping → done`, plus `abandoned`; `paused` is the hold.
- Stop shipping (shipping → working) stops the ship process and disarms any auto-merge the ship armed;
  history records it.
- A Task becomes `shipping` whenever it enters the part of its graph from which no `interactive:`
  stage remains before `end` — not only through Ship it. Example: choosing `deliver` from `demo`
  on `code`, or running `autonomous` from `start`. Ship it reaches the same state from anywhere by
  standing in at the remaining stages. Only Task APIs (`lf task run`, `lf ship`, Desktop's Task
  actions) set it; a direct `lf land` / `lf pr land` on a branch does not.
  Jack: "auto tasks become auto shipping" — a Task on a Workflow with no `interactive:` stage
  (e.g. `autonomous`) is `shipping` from its first Task run.
- These supersede the open name/disarm items above. Next: `pursue` implements the aligned state
  model, single landing engine, lazy recovery, the visible Workflow header (Ship it / Stop), and
  the gate blocker (branch `lf` must open stores holding `flow_finished` events).

### Desktop views build on #1580 (Jack Heart, 2026-10-22)

> it should be possible to look at shipping tasks (we should probably rebase onto 1580) but
> default should filter them out

- #1580 (LOO-497, Dashboards) makes *Working tasks* the repository's built-in Work query in plan
  order and adds a query language with `state:`. Build on it: the default Working query excludes
  `state:shipping`; Shipping is a query too (`state:shipping`), not a separate `SidebarFilter`
  case. The `state:` vocabulary is the new lifecycle (unstarted/working/shipping/done/abandoned,
  plus paused).
- #1580 was open, armed and BLOCKED on checks at 2026-10-22 ~07:40 PT. Take it through the
  ordinary sync once it merges; if it has not merged when this work reaches gate, stack on it.

## Implement pass 2026-10-22

Built:
- `TaskState` is the lifecycle: `unstarted | working | shipping | done | abandoned`, plus
  `paused` (Rust, Swift, fixtures; `not_ready`/`ready` → `unstarted`, `active` → `working`).
- Shipping is stored: `tasks.shipped_at` (shared WorkValue `task_shipped_at`, synced like
  `paused`) and machine-local `tasks.ship_lf_process_id` (the carrier). Set by `lf ship` and by
  choosing any route into `end` (`choose_edge_in`); handoff/stop each append a Task `Progress`
  event. Done/abandoned outrank it in `task_state_sql`; reopen clears it.
- `lf ship --stop` (Desktop "Stop shipping" in the header and context menu): clears shipping,
  SIGTERMs a live carrier, disarms a Loopflow-armed merge request
  (`clear_task_pr_merge_before_head_mutation`). The ship loop exits on `ShipStep::Stopped`.
- Deleted: `WorkflowMoveKind::Shipped`, the `shipped` move CHECK value, the history scan, the
  restart-count note.
- `TaskWorkflowHeader` is mounted in the Task detail (`WorkSurfaceView`).
- Gate blocker: `flow_finished` task events read as `GraphFinished` via serde alias (bytes kept).

Remaining (not started this pass):
- Single landing engine + lazy recovery: delete `lf ci watch` (+ launchd, `CIWatchers.swift`),
  the periodic Task check, `lf pr reconcile`'s repair path and `automation_enabled`; restart a
  dead carrier on lf's own reads. Restart today is still `lf task reconcile` (`restart_ships`).
- Rest of the state-model cleanup: `planning_state`/`planning_completed`, `abandon_intent`,
  `completion_request` folding; Linear mapping (shipping ↔ In Review).
- Desktop Shipping view on #1580 Dashboards (Jack, 2026-10-22: default Working query excludes
  `state:shipping`; Shipping is a `state:shipping` query, not a `SidebarFilter` case). #1580 is
  open; take it by sync when it lands, then delete `SidebarFilter.shipping`.

## Demo findings (2026-10-22, branch build on lf-world `portfolio`)

- Jack accepted the naming: `deliver` is the final `gate → land --wait-and-fix` graph; `lf ship`
  runs a Task's remaining steps (including `deliver`) with stand-ins ("ok that works").
- Failed: the Ship it button the design places in `TaskWorkflowHeader` is not visible. That
  header is mounted by no production view (only tests construct it; also true on main), so the
  routes and Ship it are unreachable. Only the sidebar context menu (`TaskActionItems`) offers
  Ship it. Next implement: mount the Workflow header in the Task view, or delete it.
- Dev harness: `loopflow://` links opened in the dev app resolve against the real `~/.lf`, not
  the world's `LF_HOME` (30 s timeout while copying the real store). Not checked against main.
- Gate blocker: the branch `lf` refuses Jack's real store: semantic migration check fails on
  `task_events.kind_json` rows with `flow_finished` (the Flow → Graph rename did not migrate
  them). Only a copy was read; the real store is untouched.
- World Tasks that ran a Flow read as `unstarted` on the branch; main's world builder says "a
  Flow in a checkout starts its Task". Possible regression, unverified.

## Problem

When Jack decides a Task is good to go, he wants to say so once and stop thinking about it.
Today that takes three things by hand: picking a command (`lf ship`, `lf pr land --wait-and-fix`,
`lf arm`), watching CI, and relaunching repairs. All the while, the Task sits in his working list.

- **No Task-level action.** `lf ship` is only the `lf <graph>` shorthand. It does not move the
  Task's Workflow, and it is unreachable from `design`.
- **Two models for one idea.** A Workflow (`workflow.rs`, `.lf/workflows/`) is nodes plus
  edges that name Flows. Flows *also* carry person steps: `human: true`, the
  `InteractionGraph` projection, and `human_occurrence_ids`. These are left over from #1283
  and refused at run time by `require_autonomous_steps`. This repo's `.lf/flows/kickoff-demo.yaml`
  still authors one. The live store holds 296 captured Workflows: 239 at `end` and 57 at a
  node or on an edge (`code`, `feature`, `research`, `autonomous`, `iterate`).
- **Late detection.** main's only required check is the aggregate `tests-result`. A probe on
  2026-10-21 (run 39694962829) showed it has **no check run at all** while its `needs:` jobs
  run. `project_checks` (`ops/pr.rs`) therefore reads Pending until the slowest macOS job
  finishes.
- **Dequeue strands the landing.** `classify_github_observation` maps "no auto-merge request"
  to `Unarmed`, and `reconcile_pr_landing` blocks on it with "run lf arm". PR #1613 sat 97
  minutes after `RemovedFromMergeQueueEvent reason=failed_checks`; #1628 needed a hand
  `lf arm`. A completed repair whose evidence has not changed also blocks
  ("waiting for changed evidence"), and `wait_for_landing` returns on Blocked.

## Demo

1. In Desktop, Jack opens LOO-X at `design` and clicks **Ship it**. It leaves *Working tasks*
   and appears under *Shipping* as "Building · implement". Its Task conversation stays open.
2. The `demo` step runs headless as Jack's stand-in. The Task history reads
   `demo (stand-in) → advance: "manual check of lf ship output passes"`. The row turns to
   "Landing · CI running on #1650".
3. A `rust-lint` leaf fails at minute 3, while `tests-result` has no check run. Within one
   15 s poll the row reads "Repairing · rust-lint".
4. The repair arms the PR and it enters the queue. The queue run times out
   (`checks_timed_out`), the landing re-arms on its own, and the PR merges. The Workflow
   arrives at `end`, completion settles, and LOO-X leaves Shipping. Jack did nothing.
5. In a terminal on an untracked branch: `nohup lf ship &`. A new Task appears under Shipping.
   `lf task status LOO-Y` and `tail nohup.out` show the same progress. Running `lf ship` again
   prints "LOO-Y is already shipping (process …)" and exits 0. `kill` the first one, and within
   a minute `lf task reconcile` restarts it.

## Chosen approach

> yeah that works  (Jack, 2026-10-21, choosing `interactive: review-design` over `-i` and `human:`)

Step syntax is the keyed form `interactive: <skill>`, matching `cmd:`/`graph:`/`loop:` and the
CLI's `-i` meaning. Not `human:` (the #1283 key, and AGENTS.md's voice rule).

### 1. A Workflow is a graph; `interactive:` marks where a person takes part

There is one definition kind: the graph. A step written `interactive: <skill>` is interactive. It is a
**stage**, the generalization of today's Workflow node. Builtins move to
`builtins/task/graph/`:

```yaml
# feature: design, build and deliver. You review the plan and try the change.
- kickoff
- loop: kickoff
  interactive: review-design
- graph: pursue
- loop: implement
  interactive: demo
- graph: deliver
```

```yaml
# code
- graph: pursue
- loop: implement
  interactive: demo
- graph: deliver
```

```yaml
# research: investigate and report, with no PR
- research
- loop: research
  interactive: research
```

```yaml
# deliver
- gate
- cmd: land --wait-and-fix
```

**Authoring rules (`graph/mod.rs`):**

- `interactive: <skill>` is a step kind, accepted at top level and as a `loop`'s deciding step.
- A stage's id is its skill name. Two stages with the same skill name are an error that asks
  for a mapping `{name, id}`.
- An `interactive:` decider is allowed. This lifts "a loop's deciding step is autonomous". At a stage, the
  person decides advance or repeat, just as the node's self-edge works today.
- `interactive:` inside an `xor` path is allowed (Jack, 2026-10-21: "drop the rejection"). It is a
  conditional stage: the projection shows it on its branch, routes out of the preceding stage name
  each branch, and the stored position names the stage that actually ran.
- An autonomous loop may not return across a stage.
- The `human:` key and `require_autonomous_steps` are deleted.

**Stages and routes are projected from the graph.** `InteractionGraph` already computes stages
and the transitions between them. It is promoted to the source for the Task Workflow DTO:

- **Nodes** are stages (skill, plus a description from the skill's first prose line).
- **Edges** are routes. Each route has a `from` stage, a `to` stage, and a `name`: the
  outermost composed graph of the route's first step, else that step's skill, else `end`.

From `demo` on `feature`, the routes are `pursue` (repeat, to `demo`) and `deliver` (advance, to
`end`). Those are the same names as today's edge buttons. Route names are unique per stage.

**Running a Task (`lf task run ISSUE [ROUTE]`)** starts one Graph process over a contiguous slice
of the captured steps. The slice runs from the route's first step up to the next stage, keeping
inner loop returns intact. When the runner reaches a stage it returns `GraphOutcome::Staged`,
and the Task arrives there. The GraphProcess records the
whole captured graph, and its step rows locate the slice.

**Capture and store.** `task_workflows.graph` stores the captured compiled graph
(`{name, steps: [ConcreteStep]}`) in place of a `WorkflowDefinition`:

- `node` is the stage id, `start` or `end`.
- `edge` is the route index.
- Moves keep their meaning, with `from`/`to` naming stages.

Taking up a graph still uses the Wave default, then the repository `workflow:` key. That key
stays in `.lf/config.yaml` per the shared-file rule and now names a graph. The Task is placed
at the stage named by the Task's last recorded node when the new graph has a stage with that
id. Otherwise it is placed at `start`.

**Migration (one draft).**

- Rows at `end` keep their position, with graph `{name, steps: []}`.
- Every other row is deleted while its history stays. The next `lf task run` takes up the
  default graph and lands on the same stage by the rule above.
- Builtin stage ids are `review-design`, `demo` and `research`. The draft therefore rewrites
  history `to` values for `design` → `review-design` and `findings` → `research`.
- Add `shipped` to the move kinds (see §2).

### 2. `lf ship [GRAPH]` — run to the end, standing in for the person

`lf ship` is a top-level command (the generic shorthand loses the name with `ship.yaml`). It
reads the global `--task`, then the checkout's Task. With neither, it creates a Task for this
checkout's branch and worktree: titled from the PR title, else the branch. It creates no
second worktree, and it keeps the branch and the PR.

1. **Refusals first.** It refuses only for terminal states (done, abandoned) and a missing
   checkout (`TaskRunControl`). A paused Task prints "paused; `lf task resume X`" and exits 0.
2. **Idempotent.** If a live ship process already carries the Task, print
   `LOO-X is already shipping (process P)` and exit 0. `lf task status` and `lf task reconcile`
   read liveness the same way, from the Workflow position's `lf_process_id`.
3. **Approve.** Append a `Shipped` move by Person or Conversation, with the requester's name
   and an optional `--reason`. Jack's approval *is* this move; there is no flag or column.
4. **Run to `end` in this process.** Use the Task-run machinery with `stop_at_stage = false`.
   Every `interactive:` step runs headless with an added stand-in instruction: "No one is available. You
   are standing in for <name> at <stage>. Decide as they would, record why, and continue."
   The decider's answer is advance or repeat. A stand-in answering `blocked` repeats with the
   blocker as the message. As the run passes a stage, it appends `Arrived` and `Chose` moves
   (actor Conversation) whose note is the stand-in's one-line decision. Desktop's graph and
   `lf task status` therefore track the run.
5. `lf ship <graph>` takes that graph up exactly as `lf task run X <graph>` does (a `TookUp`
   move, placed at the stage matching the Task's last node, else `start`), then ships it; the
   `Shipped` note names the graph. Naming the graph the Task is already on takes nothing up.
6. **Progress.** The run writes the ordinary Flow output to stdout/stderr, so `nohup.out` and
   `&` work, and the step Processes are visible in `lf graph list --processes`. Exit codes:
   - 0 at `end` with completion requested;
   - 130 when interrupted;
   - 1 on a crash. `run_for_task` already retries a failing Graph process three times.
7. **Pause.** Between steps the runner rereads the Task state. When it finds Paused, it exits 0
   and leaves the position on its route.

**Restart.** `lf task reconcile` already runs every minute from the installed schedule. For
each Task on this Machine that is shipping, not paused, and has no live ship process, it starts
`lf --task X ship` detached through `os_process::start_lf_session`. It starts at most one per
Task per check, and the restart count shows in the Shipping reason.

**Shipping ends** when completion settles at `end`, or when a person makes a later `Set` move
(`lf task workflow move`). Pause halts shipping without ending it.

### 3. Shipping projection

`TaskDetailSnapshot`, and from it `RoadmapTask`, gains `shipping: Option<ShippingSnapshot>`.
It is derived in Rust from the move history, process liveness, Task state and `PrLanding`, and
is never stored:

```rust
pub struct ShippingSnapshot {
    pub shipped_at: i64,
    pub state: ShippingState,         // Building | Landing | Repairing | Restarting | Paused
    pub reason: String,               // one line, always present
}
```

| Evidence | state | reason |
| --- | --- | --- |
| Live ship process, no landing recorded | Building | the running step, "(stand-in)" on a stage |
| Landing Watching | Landing | "CI running on #N", "Queued (position 2)" |
| Landing Repairing, or a repair attempt backing off | Repairing | "Repairing · rust-lint (attempt 2)" |
| Shipping, no live process | Restarting | "Ship process exited; restarts at next check (×N)" |
| Task paused | Paused | "Paused · `lf task resume X`" |

`ship_control: TaskRunControl` sits beside `run_control`. It is nil when `lf ship` would start,
and Desktop never re-derives it.

### 4. Desktop

- `TaskWorkflowHeader` gains an emphasized **Ship it** button (`task-ship`). It replaces the
  route buttons into `end`, so `demo` shows "pursue" and "Ship it". `TaskActionItems` gains
  "Ship it". Both spawn `lf --task X ship` detached through a new
  `RegistryQuery.shipTask`, the same detached launch as Task runs.
- `SidebarFilter` gains `.shipping = "Shipping"`. It lists every repository Task with
  `shipping != nil`, read from the roadmap rather than the retained working IDs.
  `WorkModel.workingTaskIDs` excludes shipping Tasks the way it already excludes paused ones.
  The Task conversation is not stopped.
- Shipping rows show `state · reason` in secondary text. No row uses failure colour, because
  nothing waits on Jack.
- `WorkflowGraph` keeps drawing nodes and edges. Its edges read `name` (was `flow`).

### 5. `land --wait-and-fix` that keeps going

The changes are in `ops/pr.rs`, `ops/pr_landing.rs` and `ops/ci_watch.rs`. The generation, lock,
incident and reservation machinery is unchanged.

**a. Early failure through required-check producers.** `RequiredProducers` maps each required
context to the job that reports it and the transitive `needs` closure of that job.

- It is parsed from the landing checkout's `.github/workflows/*.y{a,}ml`. A job is matched by
  its key or literal `name:`. Matrix runs match by base name (`rust-test (library, 3, 5)` →
  `rust-test`).
- Required contexts come from `rules/branches/{base}`, else classic protection. The parser
  `required_contexts` lives in `ops/pr.rs`; the watcher keeps its ETag-backed cache and the
  landing reads uncached.
- `MergeGateReading::from_checks` treats a failed or cancelled run as an early gate failure when
  its workflow (GraphQL `checkSuite.workflowRun.workflow.name`) is the producer and its job is
  in the closure. This holds even while the required check is absent.
- Optional checks keep today's meaning, and
  `context_states_preserve_gate_and_optional_failure_meanings` stays as written.
- When no producer resolves, the landing waits for the required check, and `lf ci` says
  "required check producer unknown".
- `ci_watch::read_gate` hands `Unreported` with failures to the landing reading. There is one
  judgment.

**b. Queue runs are not repaired early.** Settled in concept review (2026-10-22) by Jack's
"flakes dominate" evidence: a failing queue commit is usually a flake, so the landing waits for
GitHub's drop and (c) re-queues once before any repair. No queue-commit reading is built, and the
queued → Pending rule stays.

**c. Dequeue and disable are classified, then acted on.** `Unarmed` becomes
`Dequeued { head_sha, reason }`. The reason comes from the newest removal or disable event after
the landing's last arm.

| GitHub evidence | Landing does |
| --- | --- |
| `failed_checks` | Failing `merge-queue`: re-queue once, then repair |
| `merge_conflict` | Failing `required-integration` (existing path) |
| `checks_timed_out`, no event, `manual`, `manually_disabled` | Re-arm the same head mechanically (a reasonless drop is re-read after 5 s first). Pause holds it. Each re-arm appends a Task `Progress` event ("PR #N: auto-merge was turned off on GitHub at T; re-armed <head>"). |
| PR closed | Terminal; `wait_for_landing` returns held. Reopen waits until `lf pr abandon` fences the landing (see `questions.md`). |

`rearm_github` re-reads the PR. When auto-merge survived it calls `enqueuePullRequest`;
otherwise it re-enables auto-merge on the exact head, without push or sync. `LandingIo.act`
pairs `RearmPr` with `RepairCi`, and `rearm_landing` serves both re-arm and re-queue.

**d. Nothing returns to a person.**

- `wait_for_landing` returns only on Merged or a closed PR. Blocked stays a recorded state with a
  reason.
- A ci-fix `blocked` conclusion, or a completed repair with unchanged evidence, starts a new
  attempt after a backoff: 1, 2, 4… minutes, capped at 30. The incident identity takes the
  attempt number, and the prompt carries the prior summaries.
- There is still no elapsed-time cutoff (#1586). Spend shows in the attempt count; Pause is
  the off switch.

## Risk findings

| Risk | Finding |
| --- | --- |
| Pending aggregate visible? | No. `tests-result` is absent from check-runs and jobs while dependencies run (run 39694962829). Producer analysis is required. |
| Why a PR left the queue | `RemovedFromMergeQueueEvent.reason` ∈ {`failed_checks`, `checks_timed_out`, `merge_conflict`, `manual`, `merged`}; `AutoMergeDisabledEvent.reasonCode` = `manually_disabled` (#1571–#1618). |
| Queue-run checks before dequeue | `MergeQueueEntry.headCommit` and `state` are readable. |
| Can graphs express Workflows? | Yes. Loops target an earlier skill occurrence (`resolve_loop_target`), and each builtin's self-edge is a loop whose decider is the stage. `loop: implement` from `demo` resolves uniquely inside `pursue`. |
| Migration loses positions | Rows at `end` keep theirs. The other 57 are re-placed by stage id on the next run. Running edges lose their carrier, because a stopped edge is already only history. |
| Endless repair or stand-in loops | Evidence-driven with backoff. Count visible; Pause stops. Named Wave risk: agent spend on chronic flakes. |
| A stand-in demo is weaker than Jack's | Accepted by direction. Its decision and evidence stay in the Task history for later reading. |
| #1643 collision | Resolved: merged after it landed; its Workflow edge `graph` key is gone with the edge-authored Workflow. |

## Alternatives considered

- **Keep Workflow nodes/edges and add Ship it as a jump to the end edge.** This was the first
  draft. Jack rejected it: Ship it must proceed through demo, not skip it.
- **Segment graphs into anonymous sub-flows per route.** Rejected. That creates a second
  definition per route; slicing the captured steps is enough.
- **A stored shipping flag.** Rejected. The `Shipped` move is the approval, and everything else
  is derived.
- **`lf ship` detaching itself.** Rejected per Jack. The caller backgrounds it, and reconcile
  restarts a dead one.
- **Treat any failed leaf as early failure, or add a config knob naming dependencies.**
  Rejected. The first breaks optional checks; the second duplicates the workflow file.

## Scope

**User-visible outcome.** Jack ships any Task, or any branch, with one click or `lf ship`.
It runs every remaining step, with agents standing in at interactive ones. It sits under
Shipping until merge and completion, and it never asks.

**Source of truth.** The captured graph and position plus append-only moves (`Shipped`
included), Process liveness, Task state, `PrLanding` plus incidents, and GitHub. `shipping`,
`ship_control` and the stage/route DTO are derived.

**Affected surfaces.**

- Clap: `ship`; `task run [ROUTE]`; `task workflow move <stage>`; `wave workflow` now lists and
  selects graphs.
- DTOs (`tests/fixtures/dto/`, Rust and Swift tests):
  - `Workflow` edges: `flow` → `name`;
  - `WorkflowMoveKind::Shipped`;
  - `shipping`, `ship_control`.
- Store: one draft migration.
- Swift: `RoadmapTask`, `WorkModel`, `WorkProjection`, `TaskWorkView`, `WorkNavigator`,
  `WorkflowGraph`.
- Skills: `ci-fix` (queue context, no "surface the required human action"), `task_session`
  (stages and `lf ship`), and the headless surface (stand-in line).
- Docs: `delivery.md`, `execution.md`, `authoring.md`, `lf.md`, regenerated `lf-reference.md`,
  and the architecture reference.

**Absent and error states.**

- Unknown producer: wait for the required check.
- Unreadable GitHub: no verdict (unchanged).
- Graph without stages: `lf task run` runs it whole.
- Store read failure: `shipping` follows the row's unavailable-evidence path and is never
  inferred as `None`.

**Operational boundary.** One extra GraphQL read per 15 s poll while queued. Workflow YAML is
parsed per observation. Reconcile adds a liveness read per shipping Task per minute. No new
daemon.

**Exclusions.**

- Remote-Machine ship.
- Multi-PR stacks.
- Gate skill changes.
- Completing LOO-470. Recommend Jack completes it when this lands.

## Concept review (2026-10-22, headless) — vocabulary pass

Done (19a2805609, 8180f6b036): the CLI says stages, routes and graphs (`lf ship` "standing in at
each stage", `move <STAGE>`, `run ISSUE [ROUTE]`, `lf wave workflow` "graphs"); AGENTS.md,
`architecture-reference.md` and `planning.md` describe the captured graph and position; the six
`.lf/workflows` test guards are gone. `lf-reference.md` regenerated.

Second pass (2026-10-22, headless): `lf ship <graph>` was designed to run the named graph whole
beside an unreplaced captured Workflow; the code takes it up instead, the same rule as
`lf task run X <graph>`. The code's version is kept and §2.5 now says so: one take-up rule,
Desktop draws the graph actually running, and placement by stage id is Jack's "proceed from
wherever you are". No implement work follows from it.

Settled here, from accepted direction: §5b (above) is not built; `shipped_by` needs no column —
the `Shipped` move's actor (Person, or Conversation with its Session) is the approval record in a
one-person product, so Done-when reads "`Shipped` by a person".

## Delete — do not maintain

Everything on this list is gone: the authored Workflow parser and `.lf/workflows/`, the
`ship`/`deploy` graphs, `human:` (now only a retirement error), `LoadError::Workflow`, and slice
2's `Unarmed`, completed-repair block, Blocked return and `ci_watch::required_contexts`.
`WorkflowDefinition` survives only as the projected DTO. The watcher keeps its own required-check
cache because it reads through its injected REST fetch.

## Forbidden outcomes

- A Workflow definition kind beside graphs, or a reader for old captured `WorkflowDefinition`
  JSON.
- A stored shipping flag.
- Desktop computing shipping, routes or ship legality.
- A second early-failure judgment in `ci_watch`.
- Any timeout that ends observation or repair.
- A path where a shipped Task waits for Jack.
- Two buttons for one route into `end`.

## Internal slices (one PR)

1. **This slice: graphs with `interactive:`.** Make the parser change, the builtin graphs, the
   `InteractionGraph` → Workflow DTO projection, the slice runner, capture/take-up and the
   migration draft. Delete everything from the first four Delete bullets. Focused test:
   `cargo test -p loopflow --lib -- workflow flow::` and
   `cargo test -p loopflow --test integration task_workflow`.
   - A `feature` Task runs `start → review-design → (pursue) → demo`, then `deliver` takes it
     to `end`.
   - A migrated `demo` row is re-placed at `demo`.
2. **Landing.** Implemented first (2026-10-21), because #1643 rewrites slice 1's files.
   - Done: `Dequeued` from the newest queue-removal/disable event (`timelineItems`, verified on
     #1613); a removal that leaves auto-merge on is a drop; a reasonless drop is re-read after
     5 s (arm race); re-entry is `enqueuePullRequest` when auto-merge survived, else exact-head
     re-arm; `failed_checks` re-queues once, then repairs; a head nobody armed repairs
     `auto-merge`; attempts `<identity>#n` with 1→30 min backoff and earlier summaries in the
     prompt; `wait_for_landing` returns only on merge or close; Pause holds re-arm/re-queue;
     watcher's "waiting for changed evidence" memory deleted; ci-fix skill flake guidance;
     `delivery.md` updated.
   - Done (a): `RequiredProducers` from `.github/workflows` (verified against this repo's
     `ci.yml`), promoted failures set the verdict only; parser moved to `ops/pr.rs`; watcher
     hands any failed leaf to the landing check. The watcher keeps its own ETag-backed
     required-context cache beside the landing's static one.
   - Done: `lf ci watch` marks an unreported PR "required check producer unknown" when the
     checkout's workflows produce none of its required checks; a re-arm appends a Task
     `Progress` event ("PR #N: auto-merge was turned off on GitHub at T; re-armed <head>"); a
     paused or failed re-arm/re-queue blocks and stops that check (no overwrite to Watching).
   - Remaining: PR-head flake rerun before ci-fix is left to the
     ci-fix skill; completion on a merge while a Flow is paused or running is unverified.
   - Closed PRs stay terminal (no reopen): see `questions.md`.
3. **`lf ship`, `Shipped`, stand-ins, Pause, reconcile restart.** Started 2026-10-21 on today's
   Workflow edges, by operator direction, while #1643 stays open. New code lives in
   `ops/task/ship.rs` and `lf/commands/ship.rs`.
   - Done: `lf ship [GRAPH] [--reason]` (foreground until `end`); draft `drafts/ship_moves.sql`
     rebuilds `task_workflow_moves` to admit `shipped`; `WorkflowMoveKind::Shipped` (Rust,
     Swift, `task_work.json`); shipping = latest `Shipped` move not followed by `Set`, before
     `end`; idempotent on a live shipping process; paused exits 0; Pause rechecked between
     steps; a stage runs its skill headless with the stand-in line, then the route onward
     (toward `end` first); `lf task reconcile` restarts a dead ship (`--reason` "ship process
     exited; restarted"), ahead of delivery observation; `getting-started.md` names the
     `deliver` graph as `lf deliver`.
   - Done (2026-10-21, later pass): the stand-in decides advance/repeat through the decision
     output and its one line is the `Chose` note; `blocked` repeats with the blocker; a taskless
     branch gets a Task (`ship_branch_task`: `task_create` titled from the branch, then
     placement adopts the branch's existing checkout through `TaskProcessOptions.branch`);
     the default branch is refused. The edge walk stays: graph-projected routes make it the
     same as a separate start-from entry, so none was added.
   - Remaining: the `blocked` stand-in has no backoff (it repeats at once); `shipped_by` is not
     stored (no column; see `questions.md`).
4. **Projection, DTOs, Desktop, skills, docs.** Done (2026-10-21): `ShippingSnapshot`
   (`ops/task/ship.rs`, derived from the latest `Shipped` moves, ship-process liveness, Task
   state and `pending_task_landing`) and `ship_control` on `TaskDetailSnapshot`/`RoadmapTask`;
   fixtures and Swift mirrors; Desktop `task-ship` button replaces route buttons into `end`
   in the header and the graph, "Ship it" in `TaskActionItems`, `RegistryQuery.shipTask`
   (`-b --task X ship`, detached starter), `SidebarFilter.shipping`, `workingTaskIDs`
   excludes shipping Tasks, rows show `state · reason`; docs moved to stages/routes.

## Done when

- **Ship it, or `lf ship`, runs every remaining step.** On a `feature` Task at `review-design`,
  it runs to merge and completion with no person, including a headless `demo`. History shows
  `Shipped` by a person and each stand-in decision.
- **The working set.** A shipping Task is under Shipping and not under Working tasks, and its
  conversation stays open.
- **`lf ship` outside a Task.** On a bare branch it creates a Task, and Shipping shows it.
- **Background runs.** A second `lf ship` exits 0 with "already shipping". A killed ship
  restarts within a minute. Pause stops it, and resume restarts it.
- **Early failure.** A leaf failure in `tests-result`'s `needs` closure starts a repair within
  one poll of the leaf completing, while `tests-result` is absent.
- **The #1613 and #1628 patterns.** Queue removal for `failed_checks` or `checks_timed_out`,
  and a manual auto-merge disable, all continue to merge without anyone's hand.
- **One definition kind.** `.lf/workflows/`, `deploy` and `human:` are gone, and the builtin
  `feature`, `code` and `research` are graphs.

## Acceptance check at gate (headless)

```bash
cargo test -p loopflow --lib -- workflow flow:: pr_landing ops::pr::tests ci_watch
cargo test -p loopflow --lib -- task_ship
cargo test -p loopflow --test integration -- task_workflow dto_fixtures
swift test --package-path swift --filter 'DTOFixture|WorkModelTests|TaskShip|WorkflowGraph'
```

Expected: all pass.

- The landing scenario returns `Ok` with one ci-fix per incident attempt and three re-arms.
- `task_ship` uses a stub `LF_BIN` and proves:
  - stand-in moves are recorded;
  - a second ship is idempotent;
  - a pause exits;
  - reconcile restarts a dead ship;
  - a taskless branch gets a Task.
- Swift proves the Shipping filter, that Working excludes shipping Tasks, and that one Ship it
  button replaces the `deliver` route.
- The hosted merge-queue run is left to demo.

Check results (2026-10-22, implement pass): `cargo test --lib` store::sqlite/waves/ship/workflow/work/durable, task_graph_launch_tests, local_planning, work_watch pass except pre-existing failures (also failing at 8300f436a6): `session_record::agent_session_identity_is_durable_before_the_provider_starts`, `bind_preserves_prior_work_and_usage…`, three other `durable_store_tests`/`local_planning_tests` "capture does not belong to a recorded Session", `a_landing_edge_that_returns_to_its_node_can_be_taken_again`, `every_task_launch_runs_in_the_foreground_under_the_same_checks` — deferred to gate. Integration `dto_fixtures`, `world_recipes`, CLI reference pass; Swift DTOFixture/WorkNavigation/TaskHistoryFilter/RegistryQuery/DesktopHeadless/WorkModel 152 pass; clippy clean.
