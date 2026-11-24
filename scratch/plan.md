# LOO-1084: Cross-sandbox and cross-Machine steering and status visibility

## Outcome

When a Task is owned by another Machine — whether a reachable peer (`laptop -> modal-3`, `laptop -> mini`) or an unreachable sibling sandbox (`modal-2 -> modal-3..6` with no peer SSH routes) — inspecting and steering it from any Machine is truthful and actually reaches running work:

1. **Cross-Machine and sibling-sandbox steer delivery:**
   - When the owning Machine is registered on the caller and reachable over SSH, `lf steer <task> <message>` (and served `task.steer`) routes directly to the owning Machine's server (`Write::TaskSteer`), writing the comment and steer directly into its store so running workers pick it up on their next `PostToolUse` hook and idle/stopped interactive Sessions wake immediately (<1 s).
   - When the owning Machine is unreachable or not registered on the caller (such as `modal-2` steering `MTG-419` on `modal-3`), `lf steer` saves the steer comment locally and pushes it to the repository's tracker (Linear/Asana) or plan carrier (hub/Git). On the owning Machine, `lf server` (and a running `Graph` step process when no server is running) periodically ingests inbound tracker/hub/Git changes into local SQLite `steers`, touching `steer_markers::task_marker` for mid-turn `PostToolUse` delivery across all harnesses and bumping `store_revisions` so `server/wake.rs` wakes idle interactive Sessions and starts stopped primary Sessions (within ≤30 s while the Machine owns open Tasks and Linear's budget allows).
2. **Honest `lf steer` receipt:**
   - Every Task steer returns a typed `TaskSteerReceipt { identifier, steer_id, delivery }`. `delivery.reached` (`owner | tracker | here`) says where the steer sits now: in the owning Machine's store (nothing more for the caller to do), in the tracker/plan carrier waiting for the owner to sync, or only in this Machine's store (`pending_sync`). `delivery.runs_on` names the owning Machine (`null` when unowned or owned here). `delivery.sentence`, written once in Rust, says who reads it, reusing `steer_reader`'s wording for the Task's conversation and workers on the owner ("MTG-419's worker reads it at its next boundary", "the Machine server starts … to read it", "nothing reads it until the Task runs").
   - The receipt carries no comment thread; `lf task status` shows the thread.
   - CLI `lf steer <task>` prints `delivery.sentence` instead of bare `Saved <id>`.
3. **Honest cross-Machine `lf task status` and lists:**
   - Where a Task runs is one wire fact, `runs_on: Option<TaskRunsOn { machine: MachineRef, since }>`, on both `TaskRow` (`lf task status`) and `RoadmapTask` (`lf task list --json`, `lf wave status`, Desktop board), computed once in `snapshot_task_detail` from `TaskOwnership::owner_elsewhere`. `null` when this Machine owns it or nobody does. It is the structured form of the existing `runs_elsewhere` sentence ("Runs on mini since Nov 12 10:42"), which today exists only in text output.
   - `TaskDetail` adds `reading` (`here | owner | unobserved { reason }`): whose store answered the live part (Graph, Sessions, `graph.sentence`).
   - When a Task runs on a Machine registered on the caller, `lf task status` queries that Machine's server (bounded to 15 s) and returns its live `TaskDetail` with `reading.kind == "owner"` and `runs_on` filled in by the caller.
   - When the owning Machine is not registered on the caller (e.g. `modal-2` inspecting `MTG-419` on `modal-3`) or its SSH/server connection fails, `lf task status` returns `reading.kind == "unobserved"` with the exact reason, sets `task.reason` to `"Runs on <machine> since <when>; <reason>, so its Graph and Sessions are not read here"`, and sets `task.next` to `null` — never presenting a remote Task as locally idle or suggesting `lf task run`. List reads apply the same `reason`/`next` rule without trying SSH (a list never dials peers).
   - Text output (`lf task list`, `lf task status`) prints `runs_on` and `task.reason` from the wire instead of recomputing `runs_elsewhere` in each printer.

## Findings and design choices

### Root causes at kickoff

1. **Why `modal-2` saw `MTG-419..422` as locally idle with no `sentence`:**
   - `rust/loopflow-storage/src/sqlite/task_ownership.rs:413-427` (`writer_in`) only treats an owning Machine as remote when `is_my_machine_in(conn, me, machine)` is true — i.e., the owning Machine is in `declared_machines` (`lf machine add`). Ephemeral Modal sandboxes (`rust/loopflow-execution/src/rental/sandbox.py`, `rust/loopflow/src/lf/commands/rental.rs`) receive only the laptop's public key in `authorized_keys` and add no peer Machines.
   - Because `modal-3` is absent from `modal-2`'s `declared_machines`, `remote_ownership_writer` (`rust/loopflow-execution/src/machine/task.rs:185`) returned `None`, `task_machine` (`rust/loopflow/src/lf/commands/waves.rs:973`) returned `Ok(None)`, and `lf task status MTG-419 --json` fell through to local `task_detail` (`waves.rs:1403`).
   - `TaskDetail` (`rust/loopflow/src/lf/commands/task_row.rs:61`) had no field saying which Machine runs the Task (the `runs_elsewhere` sentence exists only in text output), and `snapshot_task_detail` (`waves.rs:2319-2353`) fell into `next_move_for_task` (`"No Task Graph has run; independent Sessions may still be active"`, `next: "lf task run MTG-419"`) whenever `execution.state == Idle` locally, even when `owner_elsewhere` named another Machine.
2. **Why steers from `modal-2` sat unconsumed in Linear while `modal-3..6` ran:**
   - `Write::remote` (`rust/loopflow/src/lf/commands/write.rs:1154`) routes `TaskRun`, `TaskAutoOn`, `TaskAutoOff`, `TaskClaim`, `TaskUnassign`, and `SessionResume`, but not `TaskSteer`. CLI `lf steer <task>` (`rust/loopflow/src/lf/commands/session.rs:98-118`) always writes locally, posts a Linear comment via `append_task_comment_async`, and prints bare `Saved <id>`.
   - All five harnesses (Claude, Codex, OpenCode, Pi, Antigravity) already pick up local SQLite `steers` mid-turn via `steer_markers::task_marker` and `PostToolUse` (`rust/loopflow-execution/src/context_block.rs:301-345`, `harness/context.rs`, `harness/server_connection.js`, `harness/antigravity.rs`), and `rust/loopflow/src/server/wake.rs` already wakes idle interactive Sessions and starts stopped primary Sessions whenever `store_revisions` advances for `steers`.
   - However, **nothing on the owning Machine periodically ingests inbound Linear/Asana comments or `plan_hub` records while work is running or idle:**
     - `rust/loopflow/src/server/maintenance.rs:42-56` (`start_plan_exchange`) only runs `plan_git::exchange_all` (60 s) and skips both tracker-connected repositories (`ops::tracker::connected`) and `plan_hub::exchange`.
     - `rust/loopflow/src/server/boundary.rs:194-207` (`turn_end`) calls `tracker_events::sync_repository` on `BoundaryMoment::Stop`. Managed workers can send Stop, but a long turn, an idle Session and a stopped Session supply no timely boundary.
     - `rust/loopflow/src/lf/commands/graph.rs:1254,1365` calls `sync_repository` only between Graph steps, never during a long step.
   - **Clock-skew / in-flight race in `team_changes`:** `rust/loopflow-execution/src/ops/tracker_events.rs:170-185,243` queries `updatedAt <= through` (`now`) and advances the watermark `since` to `through`. If the owning Machine's clock is ahead of Linear's server clock, or a comment lands on Linear concurrently with the query, a comment stamped before `through` after the query executes is skipped by the next `updatedAt >= through` window. Because `task_linear_ingested_comments` (`rust/loopflow-storage/src/sqlite/children.rs:1900`) and `steers(source, revision)` are strictly idempotent, a 30 s overlap recovers revisions stamped within that window. Larger clock skew is outside this guarantee.
   - **Linux container server gap:** `rust/loopflow-execution/src/installation/server.rs:195-237` starts `lf server` via `systemctl --user`, which is absent in Modal/Daytona/RunPod containers. Because `RunningGraph::spawn` (`rust/loopflow/src/lf/commands/graph.rs:1061-1076`) already runs a `tokio::select!` watch loop while an agent step executes, sharing the exact same `linear_team_checks` lock and freshness window lets `RunningGraph::spawn` ingest steers mid-step even when `lf server` is not running, with zero extra Linear requests when `lf server` is running.

### Mechanisms compared

| Mechanism | Reachable peer (`laptop -> modal-3`) | Sibling sandbox (`modal-2 -> modal-3`) | Blocked / tradeoff |
| --- | --- | --- | --- |
| **Hub relay** (`modal-2 -> laptop/mini -> modal-3`) | Works if hub knows `modal-3` | **Blocked:** Modal sandboxes have no outbound SSH private key, no Tailscale, no configured hub, and the laptop is behind NAT and sleeps. Adding a central relay daemon contradicts self-hosted peer/tracker architecture (`.lf/wave/infrastructure/GOAL.md`, `docs/dev/reference.md`). | Cannot solve sibling sandboxes; only applicable where a hub is already configured for non-tracker plans (`plan_hub`). |
| **Direct SSH when registered + server/Graph inbound sync from tracker/hub/Git** *(chosen)* | Direct `Write::TaskSteer` to owning Machine's server (<1 s), with fallback to tracker/carrier when unreachable | `modal-2` posts `<!-- loopflow-steer: -->` to Linear (or pushes to hub/Git); `modal-3`'s `lf server` (and `RunningGraph::spawn` fallback) polls team delta / hub every 30 s while work is live, writes `steers`, touches `task_marker`, and wakes `server/wake.rs` (≤30 s) | Works with zero new infrastructure, zero SSH keys on sandboxes, and also delivers human comments made directly in Linear/Asana web UI. |

### Choices made for review (headless kickoff)

1. **Direct SSH with automatic durable fallback on pre-admission failure for `lf steer <task>`** *(passed over: failing hard when a registered remote Machine is offline, as `task.run` does).* A steer is a durable Task comment; falling back to local save + tracker/carrier sync when SSH cannot connect (`Connection::open_on` fails before transmitting the request) keeps the steer moving while reporting `reached: tracker` / `here` honestly. If a failure occurs *after* transmission (`OutcomeUnknown`), `lf steer` does **not** fall back to a second local write (preventing duplicate comments) and surfaces `OutcomeUnknown`.
2. **Graceful degradation to `reading: unobserved` for one-shot `lf task status` (`--json` and text)** *(passed over: hard-failing `lf task status` when a registered peer Machine is unreachable over SSH).* Returning saved planning + `task.runs_on` + `reading: { kind: "unobserved", reason }` lets agents and people see where the Task runs, its comments, PR, and why live execution wasn't observed, instead of failing blank. (`--subscribe` still errors when a registered remote server cannot be subscribed, because a live stream cannot be established.)
3. **The steer receipt is about the steer, not the thread** *(concept review; passed over: kickoff's flattening of `TaskComments` into the receipt).* No consumer reads the thread from `lf steer <task> --json` (Swift's `RegistryQuery` sends `task.steer` without decoding the reply; no test or builtin reads it). `TaskSteerReceipt { identifier, steer_id, delivery }`; `pending_sync` becomes `delivery.reached == here`. `task.report` keeps returning `TaskComments`.
4. **`delivery.reached` is three places, not four statuses** *(concept review; passed over: `delivered_live | saved_on_owner | queued | local_only` plus a separate `reason`).* The caller acts on one distinction: is the steer on the owner yet. `delivered_live` vs `saved_on_owner` is who reads it there, which only the sentence needs; `delivered_live` also overclaims, since a live worker reads at its next boundary, not on receipt. The reason (not registered, SSH failed, tracker offline) lives in the sentence. One enum, one sentence.
5. **`runs_on`, not `owner`** *(concept review; passed over: kickoff's `TaskDetail.owner: TaskOwner` and `execution_source`).* "Owner" already names two other things on the same records: `runtime.owner` (the person) and `next_move.owner` (who moves it next). The product's existing words are "Runs on mini since …" (`runs_elsewhere`), so the field takes them. It goes on `TaskRow` and `RoadmapTask`, not only `TaskDetail`: `modal-2`'s `lf task list --json` and the Desktop board show the same Task, and they misreport it the same way today. `reading` drops the machine that `remote`/`remote_unreachable` repeated from the owner.
6. **Keep `TaskState` at its existing 6 variants and express remote-unobserved state via `runs_on`, `reading`, `task.reason`, and `task.next = null`** *(passed over: adding a 7th `TaskState::RemoteUnknown` variant across Rust, Swift Desktop/iOS, and `TaskSection`).* A Task another Machine runs is still Working; only where it is read from changes.
7. **Inbound sync cadence: poll while the Machine owns open Tasks, every `max(30 s, linear::delta_fresh_secs())`; never when it owns none; 30 s overlap lookback on `team_changes` `since`** *(concept review; passed over: kickoff's separate 30 s live / 120 s owned-idle tiers, which need a live-process classifier to save quota that `delta_fresh_secs()` already paces; and 10 s unconditional polling).* An owned idle Task has a stopped primary Session the server starts on a steer, so it deserves the same latency. `RunningGraph::spawn` keeps its mid-step poller for containers without `lf server` (Linux `register` has only `systemctl --user`), sharing the `linear_team_checks` lock so the two never double-poll.

## Source of truth

- **Task ownership:** Append-only `task_ownership` log (`rust/loopflow-storage/src/sqlite/task_ownership.rs`), replicated via tracker comments (`<!-- loopflow-ownership:... -->`) or `plan_carriers` (`plan_hub` / `plan_git`). `TaskOwnership::owner_elsewhere(&local)` is the single predicate for whether another Machine owns a Task — regardless of whether that Machine is in `declared_machines`.
- **Steer persistence and delivery:** `steers` + `steer_delivered` + `task_comments` + `tracker_actions` on each Machine, with `steer_markers::task_marker` waking mid-turn tool hooks and `store_revisions` waking `server/wake.rs`.
- **Live execution facts (Graph, `sentence`, Sessions):** Authoritative only on the owning Machine's local store and OS processes (`rust/loopflow-execution/src/store/task_work.rs`, `ops/task_execution.rs`). Never inferred on another Machine; either read live from the owning Machine's server or reported as `reading: unobserved { reason }`.
- **Delivery receipt & remote status prose:** Computed once in Rust (`SteerDelivery::sentence`, `TaskReading`, `TaskRow::reason`, `TaskRunsOn`) and carried verbatim to CLI and JSON consumers; `lf task list` and `lf task status` text stop calling `runs_elsewhere` themselves.

## Affected surfaces and consumers

- **CLI commands:**
  - `lf steer <task> <message> [--json]` (`rust/loopflow/src/lf/commands/session.rs`)
  - `lf task status [<issue>] [--json] [--subscribe]` (`rust/loopflow/src/bin/lf.rs`, `rust/loopflow/src/lf/commands/waves.rs`, `rust/loopflow/src/lf/commands/task_row.rs`)
- **Server protocol & dispatch:**
  - `Write::TaskSteer` (`rust/loopflow/src/lf/commands/write.rs`): routed via `Write::remote()` with pre-admission fallback; returns `TaskSteerReceipt`.
  - `Read::TaskStatus` (`rust/loopflow/src/lf/commands/read.rs`, `waves.rs`): returns `TaskDetail` with `task.runs_on` and `reading`, degrading to `unobserved` on unreachable/unregistered owners.
  - List reads (`open_tasks`, `RoadmapTask`): carry `runs_on` and the same `reason`/`next` rule, no SSH.
  - `server/maintenance.rs`: periodic inbound sync for active/owned repositories (tracker + `plan_hub` + `plan_git`).
  - `lf/commands/graph.rs` (`RunningGraph::spawn`): periodic mid-step inbound sync sharing the same lock/freshness window when running an agent step.
  - Server protocol revision (`rust/loopflow-execution/src/machine/wire.rs`) if needed for `TaskSteerReceipt` result shape, plus `tests/fixtures/dto/server_schema.json`.
- **Wire types & DTO fixtures (no defaults, required nullable keys use `Option::deserialize`):**
  - `TaskDetail`, `TaskRow` (`rust/loopflow/src/lf/commands/task_row.rs`) + `tests/fixtures/dto/task_detail.json` + `rust/loopflow/tests/dto_fixtures.rs`
  - `RoadmapTask.runs_on` (`waves.rs`) + `tests/fixtures/dto/roadmap_snapshot.json` and its Swift mirror (`swift/Loopflow/Models/WaveWorkMap.swift`, `DTOFixtureTests.swift`): required nullable key
  - `TaskSteerReceipt`, `SteerDelivery`, `SteerReached` (`rust/loopflow-model/src/comment.rs` or `rust/loopflow-execution/src/engine/task.rs`) + new DTO fixture `tests/fixtures/dto/task_steer_receipt.json` + `rust/loopflow/tests/dto_fixtures.rs`
  - `tests/fixtures/dto/server_schema.json` + `rust/loopflow/tests/served_reads.rs` (CLI/server equivalence for `task.steer` and `task.status`)
- **Swift / Desktop / Mobile:**
  - `swift/Loopflow/Services/RegistryQuery.swift` already encodes `task.steer` and does not decode its reply into `TaskComments`; `TaskComments` (`swift/Loopflow/Models/TaskComments.swift`) stays unchanged so `TaskStatusReading`, `ScreenReads.TaskDetail`, and `TaskCommentsView` keep decoding cleanly.
  - Desktop/Mobile `Read::TaskStatus` is `Untyped` in `server_schema.json`; `swift/LoopflowTests/ServerProtocolTests.swift` and `DTOFixtureTests.swift` must stay green.
- **No SQLite migration required:** All required tables (`steers`, `steer_delivered`, `task_comments`, `task_ownership`, `task_linear_ingested_comments`, `linear_team_checks`, `tracker_actions`, `plan_carriers`) already exist.

## Absent and error states

| Scenario | `lf task status` (`--json` & text) | `lf steer <task>` (`--json` & text) |
| --- | --- | --- |
| **Owned locally, live worker or interactive Session** | `runs_on: null`, `reading: here`, live `graph`/`sentence`/`sessions` | `reached: owner`; sentence names the live reader ("reads it at its next boundary") or the server start |
| **Owned locally, no live Session (idle/unstarted)** | `runs_on: null`, `reading: here` | `reached: owner`; sentence says nothing reads it until the Task runs, or that the server starts its stopped primary Session |
| **Owned on registered reachable remote Machine** | Queries remote server (15 s bound); returns its live `TaskDetail` with `reading: owner` and `runs_on` set | Routes `Write::TaskSteer` to remote server (15 s bound); returns the owner's receipt (`reached: owner`) with `runs_on` set and the owner's reader sentence |
| **Owned on unregistered peer (`modal-2 -> modal-3`)** | Local planning + `runs_on` + `reading: unobserved { reason: "<machine> is not registered here" }` + `task.reason` naming it + `task.next: null` + `graph: null`; `lf task list` shows the same reason/next | Saves locally + drains to tracker/carrier; `reached: tracker` (synced) or `here` (`pending_sync`); sentence says the owner is not registered here and reads it when it syncs |
| **Owned on registered remote Machine, SSH/server fails before admission** | Local planning + `reading: unobserved { reason: "<ssh/server error>" }` + `task.next: null` | Local save + tracker/carrier sync; `reached: tracker` (or `here` if sync also failed); sentence names the SSH/server failure |
| **Remote `task.steer` fails with `OutcomeUnknown` (connection lost after send)** | N/A | Returns `OutcomeUnknown` error without double-writing locally |
| **Unowned Task** | `runs_on: null`, `reading: here` (or `owner` via the Wave Machine if reachable), normal unowned reason | Saves locally + syncs; `reached: owner` when this Machine is where it would run (nothing else will read it sooner), else `tracker`/`here`; sentence says nothing reads it until someone runs it |
| **Terminal Task (`done` / `abandoned`) owned elsewhere** | `task.state` remains `done`/`abandoned`; `runs_on` still set | Saves comment/steer as today; sentence says the Task is finished |
| **Linear rate-limited / offline on owning Machine during periodic sync** | N/A | Existing `Cooldown` / `delta_fresh_secs()` paces or pauses non-urgent polling cleanly, retaining watermark and retrying on next tick; never crashes `lf server` or a running Graph step |
| **Move in flight from this Machine** | Keeps local move-in-flight status (`moving_here`), never queries unarrived target | Keeps local behavior (same `remote_ownership_writer` guard) |

## Operational boundary

- **Direct cross-Machine latency:** `lf steer <task>` and `lf task status <task>` to a reachable registered Machine complete in one SSH `lf server connect` round trip, bounded by a **15 s** deadline (never hanging for the 120 s default `ANSWER` timeout).
- **Indirect (sandbox-to-sandbox) steer delivery latency:** ≤**30 s** from tracker/hub write to local `steers` insertion and `steer_markers::task_marker` touch on a Machine that owns the open Task, while Linear's hourly budget is ≥25%; slower only as `delta_fresh_secs()` stretches.
- **Linear API quota:**
  - Idle Machines (0 owned open Tasks): **0 requests/hour**.
  - Machine owning open Tasks on 1 team: **~120 requests/hour** (2 req/min) at full budget, shared across all Tasks on that team via one `team_changes` delta and one `locks/linear-teams/<key>` file lock across `lf server` and `RunningGraph::spawn`. Sandboxes sharing one Linear key share its budget, and each Home's `delta_fresh_secs()` reads Linear's remaining-budget headers, so a fleet stretches together.
  - Below 25% remaining hourly quota, `linear::delta_fresh_secs()` automatically stretches the interval; below 10%, polling stops completely to protect `urgent` writes.
- **Clock skew resilience:** `team_changes` queries `since - 30s` (clamped at epoch) while `task_linear_ingested_comments` (`INSERT OR IGNORE`) and `steers(source, revision)` guarantee at-most-once steer creation.
- **Server-absent resilience:** A running Graph step (`RunningGraph::spawn`) still ingests inbound steers mid-step even if `lf server` is not running on the container.
- **No silent drops on restart:** `server/wake.rs` reads `store_revisions` on startup; `linear_team_checks` persists the last successful watermark in SQLite.

## Exclusions

- **No hub relay or SSH mesh between ephemeral sandboxes:** Modal sandboxes do not receive private SSH keys or peer SSH routes to sibling sandboxes.
- **No Linux container `lf server` supervisor redesign:** Replacing `systemctl --user` in `installation/server.rs` for containers is left to LOO-1081/LOO-1083; LOO-1084 ensures mid-step steer delivery works both with `lf server` and without it (`RunningGraph::spawn`).
- **No remote workspace/file/diff/shell/session viewer routing:** Owned by sibling Task LOO-1083.
- **No cross-Machine `lf steer <session-id>` routing:** Sessions remain Machine-local; cross-Machine steering targets Tasks (`lf steer <task>`).
- **No `TaskState` enum changes and no SQLite schema migrations.**

## What goes

- **Replaced/removed:**
  - `lf steer <task>` printing bare `Saved <id>` (`rust/loopflow/src/lf/commands/session.rs:115`) — replaced by `delivery.sentence`.
  - `Write::TaskSteer` returning raw `TaskComments` (`rust/loopflow/src/lf/commands/write.rs:278`) — replaced by `TaskSteerReceipt` (flattening `TaskComments` + `steer_id` + `delivery`).
  - `task_machine()` relying solely on `remote_ownership_writer` and hard-failing on SSH error (`rust/loopflow/src/lf/commands/waves.rs:973-1018`) — replaced by owner-aware remote resolution that distinguishes local, reachable remote, and unreachable/unregistered remote.
  - `snapshot_task_detail` falling through to `"No Task Graph has run; independent Sessions may still be active"` and `next: Some("lf task run ...")` for an open Task owned by another Machine (`rust/loopflow/src/lf/commands/waves.rs:2319-2353`, `task_row.rs:237-248`).
  - `maintenance::start_plan_exchange` exchanging only `plan_git` (`rust/loopflow/src/server/maintenance.rs:33-56`) — generalized to inbound sync covering tracker-connected repos (with active/owned Tasks), `plan_hub`, and `plan_git`.
- **Must survive:**
  - All existing `lf steer <session>`, `lf -b steer <session>`, `lf --wave ... steer`, and `lf --repo ... steer` behaviors and outputs (`rust/loopflow/tests/session_steers.rs`).
  - Existing `TaskComments` wire shape in `TaskStatus`, `DesktopTaskPane`, `screens::TaskDetail`, and Swift `TaskComments.swift`.
  - Existing remote `task.status` live-read behavior when a registered peer Machine IS reachable (`a_served_status_for_a_task_mini_runs_is_minis_answer` in `rust/loopflow/tests/machine_commands.rs:4546`).
  - Existing `task_ownership` write authority rules (`writer_in` for `claim`/`unassign`/`move`) and move-in-flight local status.
  - Linear budget pacing (`Cooldown`, 25% pacing, 10% urgent reserve) and echo suppression (`own_echo` in `ingest_linear_comment`).

## Demo

Two disposable Homes (`sandbox-a` and `sandbox-b`, no SSH routes between them) connected to a shared fake Linear server, plus a `laptop` Home with `sandbox-b` registered over stand-in SSH:

1. On `sandbox-b`, a Task (`MTG-419`) is claimed and running a live headless worker step (or idle managed Session).
2. On `sandbox-a`, run `lf task status MTG-419 --json`:
   - Shows `task.runs_on.machine.id` = `sandbox-b`'s Machine ID, `reading.kind == "unobserved"`, `task.reason` = `Runs on sandbox-b since …; sandbox-b is not registered here, so its Graph and Sessions are not read here`, `task.next == null`, and `graph == null`. `lf task list --json` shows the same `runs_on`, `reason` and `next`.
3. On `sandbox-a`, run `lf steer MTG-419 "stop refactoring foo"`:
   - Prints `Queued <id> in Linear for sandbox-b; sandbox-b is not registered here and reads it when it next syncs`.
   - `--json` emits `delivery.reached == "tracker"` and `delivery.runs_on.id == <sandbox-b-id>`.
4. On `sandbox-b`, without any manual command, the server's inbound sync (or `RunningGraph::spawn`'s mid-step sync) pulls the Linear comment, writes the `steers` row, touches `steer_markers::task_marker`, and the running worker's next `PostToolUse` hook receives `<lf:steers>` containing `"stop refactoring foo"` (and an idle/stopped primary Session wakes).
5. On `laptop`, run `lf task status MTG-419 --json` and `lf steer MTG-419 "also check bar"`:
   - `task status` returns `sandbox-b`'s live `graph.sentence` and `reading.kind == "owner"`.
   - `lf steer` delivers directly to `sandbox-b`'s server and prints `Saved <id> on sandbox-b; MTG-419's worker reads it at its next boundary` (`delivery.reached == "owner"`).

## Acceptance check at gate

Run focused Rust integration tests exercising the full source-of-truth path across CLI, server, fake Linear, and stand-in SSH:

```bash
env $(env | grep '^LF_' | cut -d= -f1 | sed 's/^/-u /') \
  cargo test -p loopflow --test machine_commands --test session_steers --test served_reads --test dto_fixtures -- cross_machine_steer remote_unreachable inbound_tracker_steer task_detail_says_each_fact_once
```

Expected: all tests pass with zero skipped assertions — proving:
- Sibling sandbox (`sandbox-a`, no SSH route to `sandbox-b`) gets `reading.kind == "unobserved"`, `task.runs_on` set and `task.next == null` from `lf task status --json` (and the same `runs_on`/`next` from `lf task list --json`), gets `delivery.reached == "tracker"` from `lf steer --json`, and its Linear comment is ingested by `sandbox-b`'s server and delivered to `sandbox-b`'s live worker `PostToolUse` hook and idle/stopped interactive Session.
- Registered reachable peer (`laptop -> sandbox-b`) routes `task.steer` and `task.status` directly over SSH and gets `reached: owner` / `reading: owner`.
- Unreachable registered peer (`laptop -> dead sandbox-b`) degrades `task.status` to `unobserved` and falls back `task.steer` to `reached: tracker`/`here` without double-writing on `OutcomeUnknown`.
- DTO round-trip fixtures (`task_detail.json`, `roadmap_snapshot.json`, `task_steer_receipt.json`, `server_schema.json`) match Rust and Swift serialization with zero defaults.

## Slices

### Slice 1a: Incoming Linear direction on the owning Machine (implemented)

The server checks teams for its open, locally owned Tasks. During an agent step the Graph checks only its own Task's repository. Both share the team lock and last-attempt window. A tick inside that window wakes at its expiry, so staggered readers do not postpone a due read by another 30 s. A managed worker needs the server at launch. After launch its tool hook reads the store directly, even if the server has exited. Starting a stopped conversation still requires the server.

Team reads acquire freshness before collecting issue IDs. The two ID lists come from indexed reads of linked Tasks by repository. Linear overlaps the saved watermark by 30 s. Comment revisions continue through the existing store, hook marker and wake reader.

The two-Home fixture sent a steer through the sibling's Task comment command. A comment timestamped 15 s before the owner's watermark reached the worker's tool-boundary reader and the stopped primary's startup input. Replaying the window produced no second delivery. The server and Graph made one team read between them, and made zero before the claim.

- `cargo test -p loopflow-execution --lib owned_tracker_checks_deliver_a_late_comment_to_worker_and_primary_once` passed.
- On a private Machine with outbound networking disabled, the branch CLI created and claimed a Task. Its resident server attempted two Linear reads 30 seconds apart without a foreground read. The two-Home fixture above covers the delivery itself.
- `cargo build -p loopflow --bin lf`, `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` passed.
- SQLite on 10,000 synthetic Tasks across four repositories, median of 21 warm reads: repository discovery 0.008 ms, issue IDs for one repository (2,500 rows) 1.903 ms. Both plans used `idx_tasks_repository` for Task lookups.

### Slice 1b: Other carriers and the two-Machine demo (next)

- Asana reads a repository-named project within a team. Two repositories in the same team currently share a team watermark even though their project feeds differ. Key delta admission and scope matching by the project's identity before enabling periodic Asana reads. Exercise two projects in one team.
- Add bounded hub/Git exchange for owned work. Keep the existing minute Git exchange for repositories with no local claims, which receives new plans there. Share background exchange attempts across Graphs and the server.
- Exercise the timer in a two-Home demo with a running worker and an idle or stopped primary Session. Include loss of the server after a managed worker has started.

### Slice 2: Honest `lf steer <task>` routing and `TaskSteerReceipt`
- Define `TaskSteerReceipt { identifier, steer_id, delivery }` and `SteerDelivery { reached: SteerReached (owner | tracker | here), runs_on: Option<MachineRef>, sentence }` with strict required/nullable wire fields and the single Rust-authored `sentence`.
- Generalize `steer_reader` (`commands/session.rs:142`) from one Session to a Task (its live workers, its conversation, or nothing) and move it beside `engine::session::steer`, so Session and Task steers word their reader the same way and the owner's server can author it.
- Wire `Write::TaskSteer` into `Write::remote()` (bounded 15 s SSH timeout) with pre-admission fallback to local save + synchronous tracker/carrier sync for remote-owned Tasks (no fallback on `OutcomeUnknown`).
- Share that exact path from CLI `lf steer <task>` (`commands/session.rs`) so CLI and served `task.steer` return identical `TaskSteerReceipt` JSON and CLI prints `delivery.sentence`.
- Update `tests/fixtures/dto/task_steer_receipt.json`, `server_schema.json`, `dto_fixtures.rs`, and `served_reads.rs`, and add cross-Machine steer tests (direct reachable, unreachable registered fallback, unregistered sibling sandbox, `reached: here` when tracker offline, `OutcomeUnknown` no-double-write).

### Slice 3: Honest cross-Machine status and lists (`runs_on` + `reading`)
- Add `runs_on: Option<TaskRunsOn { machine: MachineRef, since }>` to `TaskRow` and `RoadmapTask`, computed once in `snapshot_task_detail` from `owner_elsewhere` (independent of `declared_machines`); `runs_elsewhere` becomes its `Display`. Add `reading: TaskReading` (`here | owner | unobserved { reason }`) to `TaskDetail`.
- In `snapshot_task_detail`, an open Task with `runs_on` set takes `reason` "Runs on <machine> since <when>" and `next: None` before `next_move_for_task` runs; `task_detail` appends the unobserved reason. Lists and status share this.
- Update `waves.rs` (`task_machine`, `remote_task_detail`, `print_remote_task_status`, `task_detail`, `list_tasks`) and `bin/lf.rs` (`TaskCommand::Status` text output):
  - Query registered remote owners over SSH with a 15 s deadline; tag live replies `reading: owner`.
  - On unregistered peer or SSH/server failure, return `reading: unobserved { reason }` (keeping terminal `done`/`abandoned` intact).
  - Text printers read `runs_on`/`reason` from the record; delete their own `runs_elsewhere`/`runs_elsewhere_here` calls (`waves.rs:918`, `bin/lf.rs:1205`) and `runs_elsewhere_here` itself if nothing else calls it. `moving_here` stays as the move-in-flight line.
- Update `tests/fixtures/dto/task_detail.json`, `roadmap_snapshot.json` (+ Swift mirror), `server_schema.json`, `dto_fixtures.rs`, `served_reads.rs`, and `machine_commands.rs` (reachable remote, unregistered sibling sandbox, unreachable registered peer, `lf task list --json` on the sibling).

## Done when

Derived from the Wave KRs (*"advance work without getting blocked by Loopflow's plumbing"*, *"don't have to track Sessions, remind agents to continue, or chase whether Tasks landed"*):

- An agent or person on a sibling sandbox with no SSH routes to peer sandboxes runs `lf steer <remote-task> "<direction>"`, sees an honest `Queued … in Linear for <machine>; …reads it when it next syncs` receipt (and matching JSON `delivery.reached: "tracker"`), and the running worker or interactive Session on the owning sandbox receives that direction automatically within 30 s without manual intervention.
- An agent or person on a Machine that can SSH to the owning Machine runs `lf steer <remote-task> "<direction>"`, delivers it directly to the owning Machine's server in <1 s, and sees `Saved … on <machine>; <who reads it>` (falling back honestly to `tracker`/`here` if SSH cannot connect).
- Running `lf task status <remote-task>` or `lf task list` (text or `--json`) from an unreachable sibling sandbox or when a registered peer is offline names the Machine it runs on (`runs_on`) and, for status, reports `reading.kind == "unobserved"`; `task.next == null` on both, never portraying a remote Task as locally idle or unstarted.
- Idle Machines with no owned open Tasks make zero background Linear requests, and active Machines stay within Linear's existing pacing and reserve limits.
- `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, Rust DTO/server/remote/steer suites, and Swift DTO/protocol tests pass.

## Docs (written first)

- `docs/work.md` — "Steer work in flight": `lf steer` receipt line and `--json` `delivery` fields.
- `docs/dev/machines.md` — steer joins the forwarded writes (with its fallback exception); new section "Steer and read a Task another Machine runs"; `lf task list` example drops the trailing next-move text for a remote Task.

Open question: on `modal-2`, which name does `runs_on.machine` print for an unregistered `modal-3`, its public name from the ownership entry or its Machine id? The docs assume the public name (`Runs on modal-3`); the existing machines.md text says a Machine unknown to the caller's preferences shows its id.
