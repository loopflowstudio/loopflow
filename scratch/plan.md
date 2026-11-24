# LOO-1084: Cross-sandbox and cross-Machine steering and status visibility

## Outcome

When a Task is owned by another Machine — whether a reachable peer (`laptop -> modal-3`, `laptop -> mini`) or an unreachable sibling sandbox (`modal-2 -> modal-3..6` with no peer SSH routes) — inspecting and steering it from any Machine is truthful and actually reaches running work:

1. **Cross-Machine and sibling-sandbox steer delivery:**
   - When the owning Machine is registered on the caller and reachable over SSH, `lf steer <task> <message>` (and served `task.steer`) routes directly to the owning Machine's server (`Write::TaskSteer`), writing the comment and steer directly into its store so running workers pick it up on their next `PostToolUse` hook and idle/stopped interactive Sessions wake immediately (<1 s).
   - When the owning Machine is unreachable or not registered on the caller (such as `modal-2` steering `MTG-419` on `modal-3`), `lf steer` saves the steer comment locally and pushes it to the repository's tracker (Linear/Asana) or plan carrier (hub/Git). On the owning Machine, `lf server` (and a running `Graph` step process when no server is running) periodically ingests inbound tracker/hub/Git changes into local SQLite `steers`, touching `steer_markers::task_marker` for mid-turn `PostToolUse` delivery across all harnesses and bumping `store_revisions` so `server/wake.rs` wakes idle interactive Sessions and starts stopped primary Sessions (within ≤30 s while the Machine owns open Tasks and Linear's budget allows).
   - An unowned Task is never routed to a Wave/repository placement Machine (`set machine mini`) on status or steer: placement selects where future work starts, not where an unowned Task runs.
2. **Honest `lf steer` receipt:**
   - Every Task steer returns a typed `TaskSteerReceipt { identifier, steer_id, delivery }`. `delivery.reached` (`owner | shared | here`) says where the steer sits now: in the owning Machine's store (`owner`, nothing more for the caller to do), in the shared tracker or plan carrier (`shared` — Linear, Asana, Git, or hub — waiting for the owner to sync), or only in this Machine's store (`here`, sync pending). `delivery.runs_on` names the owning Machine (`null` when unowned or owned here). `delivery.sentence`, written once in Rust, says who reads it, reusing `steer_reader`'s wording for the Task's conversation and workers on the owner ("MTG-419's worker reads it at its next boundary", "the Machine server starts … to read it", "nothing reads it until the Task runs").
   - The receipt carries no comment thread; `lf task status` shows the thread.
   - CLI `lf steer <task>` prints `delivery.sentence` instead of bare `Saved <id>`.
   - Empty steer messages fail locally as `Invalid` before dialing SSH; explicit remote server refusals (`Invalid`, `NotFound`, etc.) preserve their `ErrorCode` while post-send transport/lost-reply failures return `OutcomeUnknown` without double-writing.
3. **Honest cross-Machine `lf task status` and lists:**
   - Where a Task runs is one wire fact, `runs_on: Option<TaskRunsOn { machine: MachineRef, since }>`, on both `TaskRow` (`lf task status`) and `RoadmapTask` (`lf task list --json`, `lf wave status`, Desktop board), computed once in `snapshot_task_detail` from `TaskOwnership::runs_on`. `null` when this Machine owns it or nobody does. It is the structured form of the existing `runs_elsewhere` sentence ("Runs on mini since Nov 12 10:42").
   - `TaskDetail` carries `reading` (`here | owner | unobserved { reason }`): whose store answered the live part (Graph, Sessions, `graph.sentence`). `reading.kind == "owner"` always pairs with `task.runs_on` set.
   - When a Task is claimed on a Machine registered on the caller, `lf task status` queries that Machine's server (bounded to 15 s) and returns its live `TaskDetail` with `reading.kind == "owner"` and `runs_on` filled in by the caller. Text output prints `detail.graph.sentence`, `detail.sessions`, and `detail.parent`/`detail.children` alongside the title, `Runs on …; read from its server.`, state, graph, PR, graph runs, and next command.
   - When the owning Machine is not registered on the caller (e.g. `modal-2` inspecting `MTG-419` on `modal-3`) or its SSH/server connection fails, `lf task status` returns `reading.kind == "unobserved"` with the exact reason. For open work, it sets `task.state` to `null`, `task.reason` to `"Runs on <machine> since <when>; <reason>, so its Graph and Sessions are not read here"`, `task.next` to `null`, and clears local execution fields (`graph`, `graph_processes`, `sessions`, `conversation`). For terminal (`done`/`abandoned`) work, it keeps `task.state` and never prints `"Runs on …"`.
   - List and view projections (`RoadmapTask`, `selection_states`, `lf task list`) never dial peers. For an open Task running on another Machine, `section` is `Working`, `next_move.reason` is `"Runs on <machine> since <when>"`, `actions` is `NoAction` with that reason, `selection_states` omits local execution states (`unstarted`/`idle`/`active`) while keeping `open`/`working`/`auto`, and `lf task list --state <state>` does not match an unobserved remote Task.

## Findings and design choices

### Root causes at kickoff

1. **Why `modal-2` saw `MTG-419..422` as locally idle with no `sentence`:**
   - `rust/loopflow-storage/src/sqlite/task_ownership.rs:413-427` (`writer_in`) only treats an owning Machine as remote when `is_my_machine_in(conn, me, machine)` is true — i.e., the owning Machine is in `declared_machines` (`lf machine add`). Ephemeral Modal sandboxes (`rust/loopflow-execution/src/rental/sandbox.py`, `rust/loopflow/src/lf/commands/rental.rs`) receive only the laptop's public key in `authorized_keys` and add no peer Machines.
   - Because `modal-3` is absent from `modal-2`'s `declared_machines`, `remote_ownership_writer` returned `None` and `lf task status MTG-419 --json` fell through to local `task_detail`.
   - `TaskDetail` had no field saying which Machine runs the Task, and `snapshot_task_detail` fell into `next_move_for_task` (`"No Task Graph has run; independent Sessions may still be active"`, `next: "lf task run MTG-419"`) whenever `execution.state == Idle` locally, even when `owner_elsewhere` named another Machine.
2. **Why steers from `modal-2` sat unconsumed in Linear while `modal-3..6` ran:**
   - `Write::remote` routed `TaskRun`, `TaskAutoOn`, `TaskAutoOff`, `TaskClaim`, `TaskUnassign`, and `SessionResume`, but not `TaskSteer`. CLI `lf steer <task>` always wrote locally and printed bare `Saved <id>`.
   - All five harnesses (Claude, Codex, OpenCode, Pi, Antigravity) already pick up local SQLite `steers` mid-turn via `steer_markers::task_marker` and `PostToolUse`, and `rust/loopflow/src/server/wake.rs` already wakes idle interactive Sessions and starts stopped primary Sessions whenever `store_revisions` advances for `steers`.
   - However, nothing on the owning Machine periodically ingested inbound Linear/Asana comments or `plan_hub` records while work was running or idle.
   - **Clock-skew / in-flight race in `team_changes`:** querying `updatedAt <= through` and advancing `since` to `through` skipped comments stamped before `through` concurrently with the query. A 30 s overlap lookback recovers them, relying on idempotent `task_linear_ingested_comments` and `steers(source, revision)`.
   - **Linux container server gap:** `installation/server.rs` starts `lf server` via `systemctl --user`, which is absent in Modal/Daytona/RunPod containers. Sharing `planning_scope_checks` lets `RunningGraph::spawn` ingest steers mid-step even when `lf server` is not running, with zero extra tracker requests when `lf server` is running.

### Mechanisms compared

| Mechanism | Reachable peer (`laptop -> modal-3`) | Sibling sandbox (`modal-2 -> modal-3`) | Blocked / tradeoff |
| --- | --- | --- | --- |
| **Hub relay** (`modal-2 -> laptop/mini -> modal-3`) | Works if hub knows `modal-3` | **Blocked:** Modal sandboxes have no outbound SSH private key, no Tailscale, no configured hub, and the laptop is behind NAT and sleeps. Adding a central relay daemon contradicts self-hosted peer/tracker architecture (`.lf/wave/infrastructure/GOAL.md`, `docs/dev/reference.md`). | Cannot solve sibling sandboxes; only applicable where a hub is already configured for non-tracker plans (`plan_hub`). |
| **Direct SSH when registered + server/Graph inbound sync from tracker/hub/Git** *(chosen)* | Direct `Write::TaskSteer` to owning Machine's server (<1 s), with fallback to tracker/carrier when unreachable | `modal-2` posts `<!-- loopflow-steer: -->` to Linear (or pushes to hub/Git); `modal-3`'s `lf server` (and `RunningGraph::spawn` fallback) polls team delta / hub every 30 s while work is live, writes `steers`, touches `task_marker`, and wakes `server/wake.rs` (≤30 s) | Works with zero new infrastructure, zero SSH keys on sandboxes, and also delivers human comments made directly in Linear/Asana web UI. |

### Choices made for review

1. **Direct SSH with automatic durable fallback on pre-admission failure for `lf steer <task>`** *(passed over: failing hard when a registered remote Machine is offline, as `task.run` does).* A steer is a durable Task comment; falling back to local save + tracker/carrier sync when SSH cannot connect (`Connection::open_on` fails before transmitting the request) keeps the steer moving while reporting `reached: shared` / `here` honestly. If a failure occurs *after* transmission (`OutcomeUnknown`), `lf steer` does **not** fall back to a second local write (preventing duplicate comments) and surfaces `OutcomeUnknown`. Explicit remote server refusals (`Invalid`, `NotFound`, etc.) keep their `ErrorCode`.
2. **Graceful degradation to `reading: unobserved` for one-shot `lf task status` (`--json` and text)** *(passed over: hard-failing `lf task status` when a registered peer Machine is unreachable over SSH).* Returning saved planning + `task.runs_on` + `reading: { kind: "unobserved", reason }` lets agents and people see where the Task runs, its comments, PR, and why live execution wasn't observed, instead of failing blank. (`--subscribe` still errors when a registered remote server cannot be subscribed, because a live stream cannot be established.)
3. **The steer receipt is about the steer, not the thread** *(pre-build concept review; passed over: flattening `TaskComments` into the receipt).* `TaskSteerReceipt { identifier, steer_id, delivery }`; `task.report` keeps returning `TaskComments`.
4. **`delivery.reached` is three places: `owner | shared | here`** *(post-build concept review; passed over: `owner | tracker | here`, which called Git and hub carriers `"tracker"` in contradiction to `docs/dev/planning.md`, and kickoff's four statuses).* `owner` = in the owning Machine's store; `shared` = acknowledged by the shared tracker or plan carrier (Linear, Asana, Git, or hub), waiting for the owner to sync; `here` = only in this Machine's store so far (`pending_sync`). The carrier name and who reads it live in `delivery.sentence`.
5. **`runs_on`, not `owner`** *(pre-build concept review; passed over: `TaskDetail.owner: TaskOwner` and `execution_source`).* "Owner" already names `runtime.owner` (the person) and `next_move.owner` (who moves it next). `runs_on` rides on both `TaskRow` and `RoadmapTask`.
6. **`TaskPeer` follows active claims only, never Wave placement preferences** *(post-build concept review; passed over: falling back to `task_execution_route` for unowned Tasks).* Placement selects where new Tasks start, not where unowned Tasks run (`docs/dev/machines.md`). Falling back to `task_execution_route` made unowned Tasks print `"Runs on mini; read from its server."`, emit `reading: owner` with `runs_on: null`, stall 15 s when `mini` was offline, and promise `"mini reads it when it next syncs"` even though idle Machines make zero background tracker polls. `TaskPeer` now holds `runs_on: TaskRunsOn` unconditionally.
7. **Keep `TaskState` at its existing 6 variants; make remote `RoadmapTask` lists, `selection_states`, and `actions` agree with `Working`** *(post-build concept review; passed over: leaving `selection_states` with both `unstarted` and `working`, leaving `lf task list --state unstarted` matching remote Tasks, and leaving `actions.recommended == "run"` on remote Tasks).* A Task another Machine runs is in `Working` with unobserved local execution state: `TaskRow.state` is `null`, `RoadmapTask.actions` is `no_action` with the `runs_elsewhere` reason, `selection_states` omits local `unstarted`/`idle`/`active`, and `lf task list --state <state>` does not match open remote Tasks.
8. **Inbound sync cadence: poll while the Machine owns open Tasks, every `max(30 s, linear::delta_fresh_secs())`; never when it owns none; 30 s overlap lookback on `team_changes` `since`** *(pre-build concept review; passed over: separate 30 s live / 120 s owned-idle tiers).* `RunningGraph::spawn` keeps its mid-step poller for containers without `lf server`, sharing `planning_scope_checks` so the two never double-poll.

## Source of truth

- **Task ownership:** Append-only `task_ownership` log (`rust/loopflow-storage/src/sqlite/task_ownership.rs`), replicated via tracker comments (`<!-- loopflow-ownership:... -->`) or `plan_carriers` (`plan_hub` / `plan_git`). `TaskOwnership::runs_on(&local)` is the single predicate for whether another Machine owns a Task — regardless of whether that Machine is in `declared_machines`, and never inferred from Wave placement preferences.
- **Steer persistence and delivery:** `steers` + `steer_delivered` + `task_comments` + `tracker_actions` on each Machine, with `steer_markers::task_marker` waking mid-turn tool hooks and `store_revisions` waking `server/wake.rs`.
- **Live execution facts (Graph, `sentence`, Sessions):** Authoritative only on the owning Machine's local store and OS processes (`rust/loopflow-execution/src/store/task_work.rs`, `ops/task_execution.rs`). Never inferred on another Machine; either read live from the owning Machine's server or reported as `reading: unobserved { reason }` (with `TaskRow.state: null` for open work).
- **Delivery receipt & remote status prose:** Computed once in Rust (`SteerDelivery::sentence`, `TaskReading`, `TaskRow::reason`, `TaskRunsOn`, `ops::task_ownership::runs_elsewhere`) and carried verbatim to CLI and JSON consumers.

## Affected surfaces and consumers

- **CLI commands:**
  - `lf steer <task> <message> [--json]` (`rust/loopflow/src/lf/commands/session.rs`)
  - `lf task status [<issue>] [--json] [--subscribe]` (`rust/loopflow/src/bin/lf.rs`, `rust/loopflow/src/lf/commands/waves.rs`, `rust/loopflow/src/lf/commands/task_row.rs`)
  - `lf task list [--state <state>] [--json]` (`rust/loopflow/src/lf/commands/waves.rs`)
- **Server protocol & dispatch:**
  - `Write::TaskSteer` (`rust/loopflow/src/lf/commands/write.rs`): shares the CLI's peer lookup, input validation, and pre-admission fallback; returns `TaskSteerReceipt`.
  - `Read::TaskStatus` (`rust/loopflow/src/lf/commands/read.rs`, `waves.rs`): returns `TaskDetail` with `task.runs_on` and `reading`, degrading to `unobserved` on unreachable/unregistered owners.
  - List & selection reads (`open_tasks`, `RoadmapTask`, `selections::task_selection_states`): carry `runs_on`, `section: Working`, `actions: NoAction`, no unobserved local runtime state in `selection_states` or `--state`, no SSH.
  - `server/maintenance.rs` & `server/service.rs`: periodic inbound sync (`watch_owned_work`) for owned repositories (tracker + `plan_hub` + `plan_git`), plus minute Git discovery for unowned repos (`exchange_unowned`).
  - `lf/commands/graph.rs` (`RunningGraph::spawn`): periodic mid-step inbound sync sharing `planning_scope_checks` when running an agent step.
- **Wire types & DTO fixtures (no defaults, required nullable keys use `Option::deserialize`):**
  - `TaskDetail`, `TaskRow`, `TaskReading` (`rust/loopflow/src/lf/commands/task_row.rs`) + `tests/fixtures/dto/task_detail.json` + `rust/loopflow/tests/dto_fixtures.rs`
  - `RoadmapTask.runs_on` (`waves.rs`) + `tests/fixtures/dto/roadmap_snapshot.json`
  - `TaskSteerReceipt`, `SteerDelivery`, `SteerReached { Owner, Shared, Here }` (`rust/loopflow-execution/src/engine/task.rs`) + `tests/fixtures/dto/task_steer_receipt.json` + `rust/loopflow/tests/dto_fixtures.rs`
  - `tests/fixtures/dto/server_schema.json` + `rust/loopflow/tests/served_reads.rs`
- **Swift / Desktop / Mobile:**
  - `swift/Loopflow/Services/RegistryQuery.swift` sends `task.steer` without decoding the reply into `TaskComments`; `TaskComments` stays unchanged.
  - `RoadmapTask` in Swift ignores `runs_on` and reads `section` + `actions`; `swift/LoopflowTests/ServerProtocolTests.swift` and `DTOFixtureTests.swift` must stay green.
- **One SQLite migration:** `planning_scope_checks.sql` replaces `linear_team_checks` (holding Linear, Asana and `plan:` scopes) and marks the released table leaving.

## Absent and error states

| Scenario | `lf task status` (`--json` & text) | `lf steer <task>` (`--json` & text) |
| --- | --- | --- |
| **Owned locally, live worker or interactive Session** | `runs_on: null`, `reading: here`, live `graph`/`sentence`/`sessions` | `reached: owner`; sentence names the live reader ("reads it at its next boundary") or the server start |
| **Owned locally, no live Session (idle/unstarted)** | `runs_on: null`, `reading: here` | `reached: owner`; sentence says nothing reads it until the Task runs, or that the server starts its stopped primary Session |
| **Owned on registered reachable remote Machine** | Queries remote server (15 s bound); returns its live `TaskDetail` with `reading: owner` and `runs_on` set; text prints `graph.sentence`, `sessions`, and `parent`/`children` | Validates message locally, routes `Write::TaskSteer` to remote server (15 s bound); returns the owner's receipt (`reached: owner`) with `runs_on` set and the owner's reader sentence |
| **Owned on unregistered peer (`modal-2 -> modal-3`)** | Local planning + `runs_on` + `reading: unobserved { reason: "<machine> is not registered here" }` + `task.state: null` + `task.reason` naming it + `task.next: null` + `graph: null`; `lf task list` puts it in `Working`, `actions: no_action`, excludes it from `--state unstarted` | Saves locally + drains to tracker/carrier; `reached: shared` (synced) or `here` (`pending_sync`); sentence names the carrier (Linear/Asana/Git/hub) and says the owner reads it when it next syncs |
| **Owned on registered remote Machine, SSH/server fails before admission** | Local planning + `reading: unobserved { reason: "<ssh/server error>" }` + `task.state: null` + `task.next: null` | Local save + tracker/carrier sync; `reached: shared` (or `here` if sync also failed); sentence names the SSH/server failure |
| **Remote `task.steer` fails after send (`OutcomeUnknown`) vs explicit remote refusal (`Invalid`/`NotFound`)** | N/A | Lost reply returns `OutcomeUnknown` without double-writing locally; explicit remote `Error` preserves its `ErrorCode` without double-writing |
| **Unowned Task (even when Wave has `set machine mini`)** | `runs_on: null`, `reading: here`, local state (`unstarted`), never dials `mini` | Saves locally + syncs; `reached: owner`, `runs_on: null`; sentence says nothing reads it until the Task runs; never dials `mini` |
| **Terminal Task (`done` / `abandoned`) last owned elsewhere** | If registered & reachable, reads owner for summary/Graph history (`reading: owner`); if unobserved, `task.state` stays `done`/`abandoned`, text prints no `"Runs on …"` line | Saves comment/steer; sentence says `<id> is finished` |
| **Linear rate-limited / offline on owning Machine during periodic sync** | N/A | Existing `Cooldown` / `delta_fresh_secs()` paces or pauses non-urgent polling cleanly, retaining watermark and retrying on next tick; never crashes `lf server` or a running Graph step |
| **Move in flight from this Machine** | Keeps local move-in-flight status (`moving_here`), never queries unarrived target | Keeps local behavior (`move_awaiting_arrival` guard) |

## Operational boundary

- **Direct cross-Machine latency:** `lf steer <task>` and `lf task status <task>` to a reachable registered Machine complete in one SSH `lf server connect` round trip, bounded by a **15 s** deadline.
- **Indirect (sandbox-to-sandbox) steer delivery latency:** ≤**30 s** from tracker/hub write to local `steers` insertion and `steer_markers::task_marker` touch on a Machine that owns the open Task, while Linear's hourly budget is ≥25%; slower only as `delta_fresh_secs()` stretches.
- **Linear API quota:**
  - Idle Machines (0 owned open Tasks): **0 requests/hour**.
  - Machine owning open Tasks on 1 team: **~120 requests/hour** (2 req/min) at full budget, shared across all Tasks on that team via one `team_changes` delta and one `locks/linear-teams/<key>` file lock across `lf server` and `RunningGraph::spawn`.
  - Below 25% remaining hourly quota, `linear::delta_fresh_secs()` stretches the interval; below 10%, polling stops completely to protect `urgent` writes.
- **Clock skew resilience:** `team_changes` queries `since - 30s` (clamped at epoch) while `task_linear_ingested_comments` (`INSERT OR IGNORE`) and `steers(source, revision)` guarantee at-most-once steer creation.
- **Server-absent resilience:** A running Graph step (`RunningGraph::spawn`) still ingests inbound steers mid-step even if `lf server` is not running on the container.
- **No silent drops on restart:** `server/wake.rs` reads `store_revisions` on startup; `planning_scope_checks` persists the last successful watermark in SQLite.

## Exclusions

- **No hub relay or SSH mesh between ephemeral sandboxes.**
- **No Linux container `lf server` supervisor redesign** (left to LOO-1081/LOO-1083).
- **No remote workspace/file/diff/shell/session viewer routing** (LOO-1083).
- **No cross-Machine `lf steer <session-id>` routing:** Sessions remain Machine-local; cross-Machine steering targets Tasks (`lf steer <task>`).
- **No `TaskState` enum changes and no Desktop SwiftUI badge redesign** (separate UI pass; existing Swift DTO tests stay green).

## What goes

- **Replaced/removed:**
  - `TaskPeer::resolve` falling back to `task_execution_route` for unowned Tasks and `TaskPeer.since: Option<i64>` (`rust/loopflow/src/lf/commands/work_route.rs`) — replaced by `TaskPeer { runs_on: TaskRunsOn, registered }` following `ownership.runs_on(&local)` only.
  - `SteerReached::Tracker` (`rust/loopflow-execution/src/engine/task.rs`) — renamed to `SteerReached::Shared` (`"shared"`), truthful for Linear, Asana, Git, and hub.
  - `print_remote_detail` dropping `graph.sentence`, `sessions`, `parent`, and `children`, and `bin/lf.rs` printing `"Runs on …"` for terminal unobserved Tasks.
  - `selection_states` and `lf task list --state` treating open remote Tasks as locally `unstarted`, and `RoadmapTask.actions` recommending `run` for a Task running on another Machine.
  - `lf steer <task>` printing bare `Saved <id>`, `Write::TaskSteer` returning raw `TaskComments`, and `maintenance::start_plan_exchange` skipping tracker/hub carriers (already replaced in Slices 1–3).
- **Must survive:**
  - All existing `lf steer <session>`, `lf -b steer <session>`, `lf --wave ... steer`, and `lf --repo ... steer` behaviors and outputs (`rust/loopflow/tests/session_steers.rs`).
  - Existing `TaskComments` wire shape in `TaskStatus`, `DesktopTaskPane`, `screens::TaskDetail`, and Swift `TaskComments.swift`.
  - Existing remote `task.status` live-read behavior when a registered peer Machine IS reachable (`a_served_status_for_a_task_mini_runs_is_minis_answer`).
  - Existing `task_ownership` write authority rules (`writer_in` for `claim`/`unassign`/`move`) and move-in-flight local status.
  - Linear budget pacing (`Cooldown`, 25% pacing, 10% urgent reserve) and echo suppression (`own_echo`).

## Demo

Two disposable Homes (`sandbox-a` and `sandbox-b`, no SSH routes between them) connected to a shared fake Linear server (or bare Git origin), plus a `laptop` Home with `sandbox-b` registered over stand-in SSH:

1. On `sandbox-b`, a Task (`MTG-419`) is claimed and running a live headless worker step (or idle managed Session).
2. On `sandbox-a`, run `lf task status MTG-419 --json`:
   - Shows `task.runs_on.machine.id` = `sandbox-b`'s Machine ID, `reading.kind == "unobserved"`, `task.state == null`, `task.reason` = `Runs on sandbox-b since …; sandbox-b is not registered here, so its Graph and Sessions are not read here`, `task.next == null`, and `graph == null`. `lf task list --json` shows `section: "working"`, `runs_on`, `actions.recommended: "no_action"`, and `selection_states` without `unstarted`; `lf task list --state unstarted` does not include `MTG-419`.
3. On `sandbox-a`, run `lf steer MTG-419 "stop refactoring foo"`:
   - Prints `Queued <id> in Linear for sandbox-b; <reason>. sandbox-b reads it when it next syncs` (or `in Git` without a tracker).
   - `--json` emits `delivery.reached == "shared"` and `delivery.runs_on.id == <sandbox-b-id>`.
4. On `sandbox-b`, without any manual command, the server's inbound sync (or `RunningGraph::spawn`'s mid-step sync) pulls the comment, writes the `steers` row, touches `steer_markers::task_marker`, and the running worker's next `PostToolUse` hook receives `<lf:steers>` containing `"stop refactoring foo"` (and an idle/stopped primary Session wakes).
5. On `laptop`, run `lf task status MTG-419` (text & `--json`) and `lf steer MTG-419 "also check bar"`:
   - `task status` returns `sandbox-b`'s live `graph.sentence` and `reading.kind == "owner"` (and text prints `graph.sentence`, `sessions`, and `parent`/`children`).
   - `lf steer` delivers directly to `sandbox-b`'s server and prints `sandbox-b · Saved <id>; MTG-419's worker reads it at its next boundary` (`delivery.reached == "owner"`).
6. On `laptop` with `set machine sandbox-b` on the Wave and an **unowned** Task (`MTG-500`) while `sandbox-b` is offline:
   - `lf task status MTG-500 --json` answers locally at once with `reading.kind == "here"`, `task.runs_on == null`, `task.state == "unstarted"`.
   - `lf steer MTG-500 "check timeout" --json` saves locally at once with `delivery.reached == "owner"`, `delivery.runs_on == null`, `"Saved <id>; nothing reads it until MTG-500 runs"`.

## Acceptance check at gate

Run the focused tests that exercise the full path across CLI, server, tracker stand-ins and stand-in SSH:

```bash
env $(env | grep '^LF_' | cut -d= -f1 | sed 's/^/-u /') \
  cargo test -p loopflow --test integration -- \
  cross_machine_steer a_status_read a_served_status_for_a_task_mini_runs task_detail_ task_steer_receipt < /dev/null
env $(env | grep '^LF_' | cut -d= -f1 | sed 's/^/-u /') \
  cargo test -p loopflow-execution --lib owned_tracker_checks_deliver_a_late_comment_to_worker_and_primary_once < /dev/null
env $(env | grep '^LF_' | cut -d= -f1 | sed 's/^/-u /') \
  cargo test -p loopflow-storage --lib task_selection_states < /dev/null
```

Expected: all tests pass with zero skipped assertions — proving:
- Sibling sandbox (`sandbox-a`, no SSH route to `sandbox-b`) gets `reading.kind == "unobserved"`, `task.state == null`, `task.runs_on` set and `task.next == null` from `lf task status --json`, gets `section == "working"`, `actions.recommended == "no_action"`, no `unstarted` in `selection_states` or `--state unstarted` from `lf task list`, gets `delivery.reached == "shared"` from `lf steer --json`, and its comment is ingested by `sandbox-b`'s server and delivered to `sandbox-b`'s live worker `PostToolUse` hook and idle/stopped interactive Session.
- Registered reachable peer (`laptop -> sandbox-b`) routes `task.steer` and `task.status` directly over SSH, gets `reached: owner` / `reading: owner`, and prints `graph.sentence`, `sessions`, and family in text.
- Unowned Tasks with a Wave placement Machine stay local on `task status` and `steer` (even when that Machine is offline), never dialing SSH or claiming `"Runs on …"`.
- Unreachable registered peer (`laptop -> dead sandbox-b`) degrades `task.status` to `unobserved` and falls back `task.steer` to `reached: shared`/`here` without double-writing on `OutcomeUnknown`, while empty steer input fails locally as `Invalid` and remote refusals keep their `ErrorCode`.
- DTO round-trip fixtures (`task_detail.json`, `roadmap_snapshot.json`, `task_steer_receipt.json`, `server_schema.json`) match Rust and Swift serialization with zero defaults.

## Slices

### Slice 1a: Incoming Linear direction on the owning Machine (implemented)

The server checks teams for its open, locally owned Tasks. During an agent step the Graph checks only its own Task's repository. Both share the team lock and last-attempt window. A tick inside that window wakes at its expiry, so staggered readers do not postpone a due read by another 30 s.

- `cargo test -p loopflow-execution --lib owned_tracker_checks_deliver_a_late_comment_to_worker_and_primary_once` passed.
- SQLite on 10,000 synthetic Tasks across four repositories, median of 21 warm reads: repository discovery 0.008 ms, issue IDs for one repository (2,500 rows) 1.903 ms. Both plans used `idx_tasks_repository`.

### Slice 1b: Other carriers and the two-Machine demo (implemented)

The server and active Graph steps poll each Asana repository project independently (admission by team GID + repository project name; watermark includes project GID). For owned work without a tracker, the server and running Graphs share a 30 s attempt per checkout, exchanging hub and Git concurrently with 5 s deadlines. The server's minute Git exchange still discovers plans in repositories without a local claim.

- Verified with two private Machine homes + bare Git origin (27.99 s delivery after owner server exited), Asana replacement-project fixture, hub-only fixture, and 10,000-scope primary-key lookup benchmark (0.0026 ms).

### Slice 2: Honest `lf steer <task>` routing and `TaskSteerReceipt` (implemented)

`lf steer <task>` and served `task.steer` share one path (`write::steer_task`) and return `TaskSteerReceipt { identifier, steer_id, delivery }`. Pre-admission failures fall back to local save + synchronous carrier sync; lost replies after send fail with `OutcomeUnknown` and never double-write. `linear_team_checks` migrated to `planning_scope_checks(scope, …)`.

### Slice 3: Honest cross-Machine status and lists (`runs_on` + `reading`) (implemented)

`TaskOwnership::runs_on(local)` produces `TaskRunsOn { machine: MachineRef, since }` on `RoadmapTask` and `TaskRow`. `TaskDetail.reading` is `here | owner | unobserved { reason }`. Unobserved open Tasks carry `TaskRow.state: null` and `TaskRow.next: null`, and `TaskSection::of` places work held on another Machine in `Working`. Subscriptions annotate the owner's frames with `reading: owner` and the caller's `runs_on` before field selection.

### QA checks (at `8092a8ef19`, then `53b19786df`)

- [Walkthrough and transcripts](qa.html): real loopback OpenSSH and a bare Git origin, three private Machines, with one Task carried forward from installed 0.16.0. Direct CLI and served receipts reached the owner; Git delivered after an SSH outage and retried a locally saved steer after Git returned; stalled SSH handshake returned saved status in 11.36 s.

### Slice 4: Claim-based status, selection and steering (implemented)

Status and steering resolve peers from the Task's active claim. A Wave placement preference first matters when work starts. Local selections, state filters, Task rows and recommendations use the same rule for which lifecycle states this Machine can see. A remote status reply shows the owner's Graph sentence, Sessions and family. A terminal Task retains its standing when its owner cannot answer.

Steer validation runs before peer lookup. The existing `Error::refused` already carries a peer's typed rejection, including its data, through the served write. The SSH fixtures distinguish that reply from losing the acknowledgement after a write. Git, hub and tracker acknowledgements all use `shared`.

On November 23 the filtered integration run passed all 15 cases, including five move and resume regressions. The storage selection test, Linear late-comment delivery test and four documented-command checks passed. `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` passed. Regenerating the schema with the branch CLI changed one enum value.

After merging main (`00d14c493b`, #1973, which rewrote Task order in `selections.rs`) at `893907a8fb`, the acceptance filter under "Acceptance check at gate" passed its 10 integration cases and the storage selection test. `cargo fmt --check` passed on the merged head; clippy and the Linear late-comment test were not rerun there (gate).

## Remaining

- Publish so CI runs `served_reads`, `binary(lf)`, the full `test(machine_commands::)` and the Swift DTO/protocol tests (`DTOFixtureTests`, `ServerProtocolTests`).
- Watch `a_status_read_for_a_task_mini_runs_shows_a_rename_made_there` under CI load (15 s stand-in SSH round trip).
- Open from QA at `53b19786df` ([qa.html](qa.html)): `RoadmapTask.lifecycle` stays `unstarted` (and `runtime`/`execution` reasons say "No Task Graph has run") for open Tasks running elsewhere; Desktop's `TaskRowStatus(task:)` would label them "Unstarted" in Working. A non-null wire field: needs a DTO decision with the Desktop pass.

## Done when

Derived from the Wave KRs (*"advance work without getting blocked by Loopflow's plumbing"*, *"don't have to track Sessions, remind agents to continue, or chase whether Tasks landed"*):

- An agent or person on a sibling sandbox with no SSH routes to peer sandboxes runs `lf steer <remote-task> "<direction>"`, sees an honest `Queued … in Linear/Git for <machine>; …reads it when it next syncs` receipt (and matching JSON `delivery.reached: "shared"`), and the running worker or interactive Session on the owning sandbox receives that direction automatically within 30 s without manual intervention.
- An agent or person on a Machine that can SSH to the owning Machine runs `lf steer <remote-task> "<direction>"`, delivers it directly to the owning Machine's server in <1 s, and sees `<machine> · Saved …; <who reads it>` (`delivery.reached: "owner"`, falling back honestly to `shared`/`here` if SSH cannot connect before admission).
- From an unreachable sibling sandbox or an offline registered peer, `lf task status <remote-task>` names the Machine and reports `reading.kind == "unobserved"` with null state and next command for open work (and keeps terminal state without `"Runs on …"` for finished work). `lf task list` names the Machine, places open remote work in `Working` with `actions: no_action`, and excludes it from `--state unstarted` and `selection_states: unstarted`.
- Unowned Tasks with a Wave placement preference read and steer locally without dialing SSH or claiming `"Runs on …"`.
- Reachable remote `lf task status` text prints the owner's `graph.sentence`, `sessions`, and `parent`/`children`.
- Idle Machines with no owned open Tasks make zero background Linear requests, and active Machines stay within Linear's existing pacing and reserve limits.
- `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, Rust DTO/server/remote/steer suites, and Swift DTO/protocol tests pass.

## Docs

- `docs/work.md` — "Steer work in flight": `lf steer` receipt line and `--json` `delivery` fields (`owner | shared | here`).
- `docs/dev/machines.md` — steer joins the forwarded writes (with its fallback exception); section "Steer and read a Task another Machine runs"; remote status and list projection rules.
