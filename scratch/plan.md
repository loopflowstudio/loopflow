# LOO-1084: Cross-sandbox and cross-Machine steering and status visibility

## Outcome

When a Task is owned by another Machine — whether a reachable peer (`laptop -> modal-3`, `laptop -> mini`) or an unreachable sibling sandbox (`modal-2 -> modal-3..6` with no peer SSH routes) — inspecting and steering it from any Machine is truthful and actually reaches running work:

1. **Cross-Machine and sibling-sandbox steer delivery:**
   - When the owning Machine is registered on the caller and reachable over SSH, `lf steer <task> <message>` (and served `task.steer`) routes directly to the owning Machine's server (`Write::TaskSteer`), writing the comment and steer directly into its store so running workers pick it up on their next `PostToolUse` hook and idle/stopped interactive Sessions wake immediately (<1 s).
   - When the owning Machine is unreachable or not registered on the caller (such as `modal-2` steering `MTG-419` on `modal-3`), `lf steer` saves the steer comment locally and pushes it to the repository's tracker (Linear/Asana) or plan carrier (hub/Git). On the owning Machine, `lf server` (and a running `Graph` step process when no server is running) periodically ingests inbound tracker/hub/Git changes into local SQLite `steers`, touching `steer_markers::task_marker` for mid-turn `PostToolUse` delivery across all harnesses and bumping `store_revisions` so `server/wake.rs` wakes idle interactive Sessions and starts stopped primary Sessions (within ≤30 s while work is live).
2. **Honest `lf steer` receipt:**
   - Every Task steer returns a typed `TaskSteerReceipt` (flattening `TaskComments` plus `steer_id` and `delivery`) whose `delivery.status` (`delivered_live | saved_on_owner | queued | local_only`), `delivery.owner_machine`, `delivery.reason`, and single Rust-authored `delivery.sentence` state honestly whether the steer reached a live reader on the owning Machine, was saved on the owning Machine with no live Session, was queued in the tracker/plan carrier without confirmed direct delivery to the owner, or remained local-only (`pending_sync`).
   - CLI `lf steer <task>` prints `delivery.sentence` instead of bare `Saved <id>`.
3. **Honest cross-Machine `lf task status`:**
   - `TaskDetail` (`lf task status --json` and served `task.status`) carries `owner` (`Option<TaskOwner>`) and `execution_source` (`local | remote | remote_unreachable`).
   - When a Task is owned by another Machine registered on the caller, `lf task status` queries that Machine's server (bounded to 15 s) and returns its live `TaskDetail` with `execution_source.kind == "remote"`.
   - When the owning Machine is not registered on the caller (e.g. `modal-2` inspecting `MTG-419` on `modal-3`) or its SSH/server connection fails, `lf task status` returns `execution_source.kind == "remote_unreachable"` with the owning Machine and exact reason, sets `task.reason` to `"Runs on <machine> (<reason>; live Graph and Session state unobserved)"`, and sets `task.next` to `null` — never presenting an unreachable remote Task as locally idle or suggesting `lf task run`. CLI text output prints the same remote-owner and reachability status.

## Findings and design choices

### Root causes verified in source

1. **Why `modal-2` saw `MTG-419..422` as locally idle with no `sentence`:**
   - `rust/loopflow-storage/src/sqlite/task_ownership.rs:413-427` (`writer_in`) only treats an owning Machine as remote when `is_my_machine_in(conn, me, machine)` is true — i.e., the owning Machine is in `declared_machines` (`lf machine add`). Ephemeral Modal sandboxes (`rust/loopflow-execution/src/rental/sandbox.py`, `rust/loopflow/src/lf/commands/rental.rs`) receive only the laptop's public key in `authorized_keys` and add no peer Machines.
   - Because `modal-3` is absent from `modal-2`'s `declared_machines`, `remote_ownership_writer` (`rust/loopflow-execution/src/machine/task.rs:185`) returned `None`, `task_machine` (`rust/loopflow/src/lf/commands/waves.rs:973`) returned `Ok(None)`, and `lf task status MTG-419 --json` fell through to local `task_detail` (`waves.rs:1403`).
   - `TaskDetail` (`rust/loopflow/src/lf/commands/task_row.rs:61`) had no `owner` or `execution_source` field, and `snapshot_task_detail` (`waves.rs:2319-2353`) fell into `next_move_for_task` (`"No Task Graph has run; independent Sessions may still be active"`, `next: "lf task run MTG-419"`) whenever `execution.state == Idle` locally, even when `owner_elsewhere` named another Machine.
2. **Why steers from `modal-2` sat unconsumed in Linear while `modal-3..6` ran:**
   - `Write::remote` (`rust/loopflow/src/lf/commands/write.rs:1154`) routes `TaskRun`, `TaskAutoOn`, `TaskAutoOff`, `TaskClaim`, `TaskUnassign`, and `SessionResume`, but not `TaskSteer`. CLI `lf steer <task>` (`rust/loopflow/src/lf/commands/session.rs:98-118`) always writes locally, posts a Linear comment via `append_task_comment_async`, and prints bare `Saved <id>`.
   - All five harnesses (Claude, Codex, OpenCode, Pi, Antigravity) already pick up local SQLite `steers` mid-turn via `steer_markers::task_marker` and `PostToolUse` (`rust/loopflow-execution/src/context_block.rs:301-345`, `harness/context.rs`, `harness/server_connection.js`, `harness/antigravity.rs`), and `rust/loopflow/src/server/wake.rs` already wakes idle interactive Sessions and starts stopped primary Sessions whenever `store_revisions` advances for `steers`.
   - However, **nothing on the owning Machine periodically ingests inbound Linear/Asana comments or `plan_hub` records while work is running or idle:**
     - `rust/loopflow/src/server/maintenance.rs:42-56` (`start_plan_exchange`) only runs `plan_git::exchange_all` (60 s) and skips both tracker-connected repositories (`ops::tracker::connected`) and `plan_hub::exchange`.
     - `rust/loopflow/src/server/boundary.rs:194-207` (`turn_end`) calls `tracker_events::sync_repository` only on `BoundaryMoment::Stop` — which headless Graph workers never send (`!kind.is_managed()` in `context_block.rs:331`), and idle/stopped Sessions never send.
     - `rust/loopflow/src/lf/commands/graph.rs:1254,1365` calls `sync_repository` only between Graph steps, never during a long step.
   - **Clock-skew / in-flight race in `team_changes`:** `rust/loopflow-execution/src/ops/tracker_events.rs:170-185,243` queries `updatedAt <= through` (`now`) and advances the watermark `since` to `through`. If the owning Machine's clock is ahead of Linear's server clock, or a comment lands on Linear concurrently with the query, a comment stamped before `through` after the query executes is skipped by the next `updatedAt >= through` window. Because `task_linear_ingested_comments` (`rust/loopflow-storage/src/sqlite/children.rs:1900`) and `steers(source, revision)` are strictly idempotent, overlapping `since` backward by 30 s eliminates dropped comments with zero duplicate steers.
   - **Linux container server gap:** `rust/loopflow-execution/src/installation/server.rs:195-237` starts `lf server` via `systemctl --user`, which is absent in Modal/Daytona/RunPod containers. Because `RunningGraph::spawn` (`rust/loopflow/src/lf/commands/graph.rs:1061-1076`) already runs a `tokio::select!` watch loop while an agent step executes, sharing the exact same `linear_team_checks` lock and freshness window lets `RunningGraph::spawn` ingest steers mid-step even when `lf server` is not running, with zero extra Linear requests when `lf server` is running.

### Mechanisms compared

| Mechanism | Reachable peer (`laptop -> modal-3`) | Sibling sandbox (`modal-2 -> modal-3`) | Blocked / tradeoff |
| --- | --- | --- | --- |
| **Hub relay** (`modal-2 -> laptop/mini -> modal-3`) | Works if hub knows `modal-3` | **Blocked:** Modal sandboxes have no outbound SSH private key, no Tailscale, no configured hub, and the laptop is behind NAT and sleeps. Adding a central relay daemon contradicts self-hosted peer/tracker architecture (`.lf/wave/infrastructure/GOAL.md`, `docs/dev/reference.md`). | Cannot solve sibling sandboxes; only applicable where a hub is already configured for non-tracker plans (`plan_hub`). |
| **Direct SSH when registered + server/Graph inbound sync from tracker/hub/Git** *(chosen)* | Direct `Write::TaskSteer` to owning Machine's server (<1 s), with fallback to tracker/carrier when unreachable | `modal-2` posts `<!-- loopflow-steer: -->` to Linear (or pushes to hub/Git); `modal-3`'s `lf server` (and `RunningGraph::spawn` fallback) polls team delta / hub every 30 s while work is live, writes `steers`, touches `task_marker`, and wakes `server/wake.rs` (≤30 s) | Works with zero new infrastructure, zero SSH keys on sandboxes, and also delivers human comments made directly in Linear/Asana web UI. |

### Choices made for review (headless kickoff)

1. **Direct SSH with automatic durable fallback on pre-admission failure for `lf steer <task>`** *(passed over: failing hard when a registered remote Machine is offline, as `task.run` does).* A steer is a durable Task comment; falling back to local save + tracker/carrier sync when SSH cannot connect (`Connection::open_on` fails before transmitting the request) keeps the steer moving while reporting `queued` / `local_only` honestly. If a failure occurs *after* transmission (`OutcomeUnknown`), `lf steer` does **not** fall back to a second local write (preventing duplicate comments) and surfaces `OutcomeUnknown`.
2. **Graceful degradation to `RemoteUnreachable` for one-shot `lf task status` (`--json` and text)** *(passed over: hard-failing `lf task status` when a registered peer Machine is unreachable over SSH).* Returning saved planning + `owner` + `execution_source: { kind: "remote_unreachable", machine, reason }` lets agents and people see who owns the Task, its comments, PR, and why live execution wasn't observed, instead of failing blank. (`--subscribe` still errors when a registered remote server cannot be subscribed, because a live stream cannot be established.)
3. **Flatten `TaskComments` in `TaskSteerReceipt` rather than adding `delivery` to `TaskComments` or breaking top-level `comments` keys** *(passed over: mutating `TaskComments`, which is also embedded in read-only `TaskDetail`/`DesktopTaskPane`, or wrapping under a breaking `thread` key).* `TaskSteerReceipt` keeps `comments`, `identifier`, `pending_sync`, `conflicts`, `refresh_error` at top level and adds required `steer_id` and `delivery`.
4. **Keep `TaskState` at its existing 6 variants and express remote-unobserved state via `TaskDetail.owner`, `TaskDetail.execution_source`, `task.reason`, and `task.next = null`** *(passed over: adding a 7th `TaskState::RemoteUnknown` variant across Rust, Swift Desktop/iOS, and `TaskSection`).* Avoids cross-app enum churn while making remote-unobserved status explicit on both `TaskDetail` and `TaskRow.reason`/`next`.
5. **Inbound sync cadence: 30 s when the Machine has live local Task execution, 120 s when it owns open Tasks with no live process, 0 when it owns no open Tasks; 30 s overlap lookback on `team_changes` `since`.** *(passed over: 10 s unconditional polling on every Machine, which burns Linear quota on idle fleets, or no fallback in `RunningGraph::spawn`, which leaves containers without `lf server` unable to receive mid-step steers).* Respects existing `linear::delta_fresh_secs()` pacing (<25% budget) and 10% urgent-only reserve automatically.

## Source of truth

- **Task ownership:** Append-only `task_ownership` log (`rust/loopflow-storage/src/sqlite/task_ownership.rs`), replicated via tracker comments (`<!-- loopflow-ownership:... -->`) or `plan_carriers` (`plan_hub` / `plan_git`). `TaskOwnership::owner_elsewhere(&local)` is the single predicate for whether another Machine owns a Task — regardless of whether that Machine is in `declared_machines`.
- **Steer persistence and delivery:** `steers` + `steer_delivered` + `task_comments` + `tracker_actions` on each Machine, with `steer_markers::task_marker` waking mid-turn tool hooks and `store_revisions` waking `server/wake.rs`.
- **Live execution facts (Graph, `sentence`, Sessions):** Authoritative only on the owning Machine's local store and OS processes (`rust/loopflow-execution/src/store/task_work.rs`, `ops/task_execution.rs`). Never inferred on another Machine; either read live from the owning Machine's server or explicitly reported as `RemoteUnreachable`.
- **Delivery receipt & remote status prose:** Computed once in Rust (`SteerDelivery::sentence`, `TaskExecutionObservation`, `TaskRow::reason`) and carried verbatim to CLI and JSON consumers.

## Affected surfaces and consumers

- **CLI commands:**
  - `lf steer <task> <message> [--json]` (`rust/loopflow/src/lf/commands/session.rs`)
  - `lf task status [<issue>] [--json] [--subscribe]` (`rust/loopflow/src/bin/lf.rs`, `rust/loopflow/src/lf/commands/waves.rs`, `rust/loopflow/src/lf/commands/task_row.rs`)
- **Server protocol & dispatch:**
  - `Write::TaskSteer` (`rust/loopflow/src/lf/commands/write.rs`): routed via `Write::remote()` with pre-admission fallback; returns `TaskSteerReceipt`.
  - `Read::TaskStatus` (`rust/loopflow/src/lf/commands/read.rs`, `waves.rs`): returns `TaskDetail` with `owner` and `execution_source`, degrading to `RemoteUnreachable` on unreachable/unregistered owners.
  - `server/maintenance.rs`: periodic inbound sync for active/owned repositories (tracker + `plan_hub` + `plan_git`).
  - `lf/commands/graph.rs` (`RunningGraph::spawn`): periodic mid-step inbound sync sharing the same lock/freshness window when running an agent step.
  - Server protocol revision (`rust/loopflow-execution/src/machine/wire.rs`) if needed for `TaskSteerReceipt` result shape, plus `tests/fixtures/dto/server_schema.json`.
- **Wire types & DTO fixtures (no defaults, required nullable keys use `Option::deserialize`):**
  - `TaskDetail` (`rust/loopflow/src/lf/commands/task_row.rs`) + `tests/fixtures/dto/task_detail.json` + `rust/loopflow/tests/dto_fixtures.rs`
  - `TaskSteerReceipt`, `SteerDelivery`, `SteerDeliveryStatus` (`rust/loopflow-model/src/comment.rs` or `rust/loopflow-execution/src/engine/task.rs`) + new DTO fixture `tests/fixtures/dto/task_steer_receipt.json` + `rust/loopflow/tests/dto_fixtures.rs`
  - `tests/fixtures/dto/server_schema.json` + `rust/loopflow/tests/served_reads.rs` (CLI/server equivalence for `task.steer` and `task.status`)
- **Swift / Desktop / Mobile:**
  - `swift/Loopflow/Services/RegistryQuery.swift` already encodes `task.steer` and does not decode its reply into `TaskComments`; `TaskComments` (`swift/Loopflow/Models/TaskComments.swift`) stays unchanged so `TaskStatusReading`, `ScreenReads.TaskDetail`, and `TaskCommentsView` keep decoding cleanly.
  - Desktop/Mobile `Read::TaskStatus` is `Untyped` in `server_schema.json`; `swift/LoopflowTests/ServerProtocolTests.swift` and `DTOFixtureTests.swift` must stay green.
- **No SQLite migration required:** All required tables (`steers`, `steer_delivered`, `task_comments`, `task_ownership`, `task_linear_ingested_comments`, `linear_team_checks`, `tracker_actions`, `plan_carriers`) already exist.

## Absent and error states

| Scenario | `lf task status` (`--json` & text) | `lf steer <task>` (`--json` & text) |
| --- | --- | --- |
| **Owned locally, live worker or interactive Session** | `execution_source: Local`, live `graph`/`sentence`/`sessions` | `delivery.status: delivered_live`; sentence names live reader(s) or server wake |
| **Owned locally, no live Session (idle/unstarted)** | `execution_source: Local`, local `graph`/`sessions` | `delivery.status: saved_on_owner`; sentence states no live Session and waits for next run/start (or server starts stopped primary Session -> `delivered_live`) |
| **Owned on registered reachable remote Machine** | Queries remote server (15 s bound); returns remote live `TaskDetail` with `execution_source: Remote { machine }` and labeled `owner` | Routes `Write::TaskSteer` to remote server (15 s bound); returns remote `TaskSteerReceipt` (`delivered_live` or `saved_on_owner`) naming target Machine |
| **Owned on unregistered peer (`modal-2 -> modal-3`)** | Returns local planning + `owner` + `execution_source: RemoteUnreachable { machine, reason }` + `task.reason` naming remote owner + `task.next: null` + `graph: null` | Saves locally + drains to tracker/carrier; `delivery.status: queued` (if synced) or `local_only` (if `pending_sync`), `owner_machine` set, `reason` explains peer is not registered |
| **Owned on registered remote Machine, SSH/server fails before admission** | Falls back to local planning + `execution_source: RemoteUnreachable { machine, reason: "<ssh/server error>" }` + `task.next: null` | Falls back to local save + tracker/carrier sync; `delivery.status: queued` (or `local_only` if sync also failed), `reason` names SSH/server failure |
| **Remote `task.steer` fails with `OutcomeUnknown` (connection lost after send)** | N/A | Returns `OutcomeUnknown` error without double-writing locally |
| **Unowned Task** | `owner: null`, `execution_source: Local` (or remote Wave Machine if reachable), normal unowned reason | Saves locally + syncs to tracker/carrier; `owner_machine: null`, `delivery.status: saved_on_owner` (or `local_only` if `pending_sync`) |
| **Terminal Task (`done` / `abandoned`) owned elsewhere** | `task.state` remains `done`/`abandoned`; `owner` and `execution_source` still populated honestly | Saves comment/steer as today; receipt states Task is finished / no live Session |
| **Linear rate-limited / offline on owning Machine during periodic sync** | N/A | Existing `Cooldown` / `delta_fresh_secs()` paces or pauses non-urgent polling cleanly, retaining watermark and retrying on next tick; never crashes `lf server` or a running Graph step |
| **Move in flight from this Machine** | Keeps local move-in-flight status (`moving_here`), never queries unarrived target | Keeps local behavior (same `remote_ownership_writer` guard) |

## Operational boundary

- **Direct cross-Machine latency:** `lf steer <task>` and `lf task status <task>` to a reachable registered Machine complete in one SSH `lf server connect` round trip, bounded by a **15 s** deadline (never hanging for the 120 s default `ANSWER` timeout).
- **Indirect (sandbox-to-sandbox) steer delivery latency:** ≤**30 s** from tracker/hub write to local `steers` insertion and `steer_markers::task_marker` touch on an owning Machine with live local execution (running Graph step or live AgentProcess), and ≤**120 s** on a Machine holding only idle/stopped open Tasks, while Linear's hourly budget is ≥25%.
- **Linear API quota:**
  - Idle Machines (0 owned open Tasks): **0 requests/hour**.
  - Active sandbox (live execution on 1 team): **~120 requests/hour** (2 req/min), shared across all Tasks on that team via one `team_changes` delta and one `locks/linear-teams/<key>` file lock across `lf server` and `RunningGraph::spawn`.
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
   - Shows `owner.machine.id` = `sandbox-b`'s Machine ID, `execution_source.kind == "remote_unreachable"`, `task.reason` naming `sandbox-b` as unreachable with live state unobserved, `task.next == null`, and `graph == null`.
3. On `sandbox-a`, run `lf steer MTG-419 "stop refactoring foo"`:
   - Prints `Queued <id> in Linear for <sandbox-b-id> (not directly reachable; delivery unconfirmed until <sandbox-b-id> syncs)`.
   - `--json` emits `delivery.status == "queued"` and `delivery.owner_machine.id == <sandbox-b-id>`.
4. On `sandbox-b`, without any manual command, the server's inbound sync (or `RunningGraph::spawn`'s mid-step sync) pulls the Linear comment, writes the `steers` row, touches `steer_markers::task_marker`, and the running worker's next `PostToolUse` hook receives `<lf:steers>` containing `"stop refactoring foo"` (and an idle/stopped primary Session wakes).
5. On `laptop`, run `lf task status MTG-419 --json` and `lf steer MTG-419 "also check bar"`:
   - `task status` returns `sandbox-b`'s live `graph.sentence` and `execution_source.kind == "remote"`.
   - `lf steer` delivers directly to `sandbox-b`'s server and prints `Delivered <id> to sandbox-b; ...` (`delivery.status == "delivered_live"`).

## Acceptance check at gate

Run focused Rust integration tests exercising the full source-of-truth path across CLI, server, fake Linear, and stand-in SSH:

```bash
env $(env | grep '^LF_' | cut -d= -f1 | sed 's/^/-u /') \
  cargo test -p loopflow --test machine_commands --test session_steers --test served_reads --test dto_fixtures -- cross_machine_steer remote_unreachable inbound_tracker_steer task_detail_says_each_fact_once
```

Expected: all tests pass with zero skipped assertions — proving:
- Sibling sandbox (`sandbox-a`, no SSH route to `sandbox-b`) gets `execution_source.kind == "remote_unreachable"` and `task.next == null` from `lf task status --json`, gets `delivery.status == "queued"` from `lf steer --json`, and its Linear comment is ingested by `sandbox-b`'s server and delivered to `sandbox-b`'s live worker `PostToolUse` hook and idle/stopped interactive Session.
- Registered reachable peer (`laptop -> sandbox-b`) routes `task.steer` and `task.status` directly over SSH and gets `delivered_live` / `remote`.
- Unreachable registered peer (`laptop -> dead sandbox-b`) degrades `task.status` to `remote_unreachable` and falls back `task.steer` to `queued`/`local_only` without double-writing on `OutcomeUnknown`.
- DTO round-trip fixtures (`task_detail.json`, `task_steer_receipt.json`, `server_schema.json`) match Rust serialization with zero defaults.

## Slices

### Slice 1 (de-risks the hardest part first): Live inbound steer ingestion & wake on the owning Machine
- Add the 30 s overlap lookback on `team_changes` `since` (`rust/loopflow-execution/src/ops/tracker_events.rs`) and verify idempotent deduplication in `task_linear_ingested_comments` + `own_echo`.
- Add one shared inbound-sync entry point (`ops::planning_sync::sync_active_work` / `ops::tracker_events`) that identifies repositories with live local Task execution vs idle owned open Tasks, runs `sync_repository` for tracker-connected repos (respecting `delta_fresh_secs()` and `locks/linear-teams`), and runs `plan_hub::exchange` (alongside existing `plan_git::exchange_all`) for non-tracker repos.
- Wire it into `rust/loopflow/src/server/maintenance.rs` (30 s live / 120 s owned-idle / 0 idle) **and** into `RunningGraph::spawn`'s `tokio::select!` watch loop (`rust/loopflow/src/lf/commands/graph.rs:1061`, 30 s interval during `StepKind::Agent`, non-blocking to interrupt checks).
- **Proves:** A steer comment posted to Linear (or hub) from an isolated sibling sandbox reaches a running headless worker's `PostToolUse` hook AND wakes an idle/stopped primary Session on the owning Machine automatically, with zero extra Linear requests when idle or when `lf server` and a Graph run coexist.

### Slice 2: Honest `lf steer <task>` routing and `TaskSteerReceipt`
- Define `TaskSteerReceipt`, `SteerDelivery`, and `SteerDeliveryStatus` with strict required/nullable wire fields and single Rust-authored `sentence`.
- Add local reader inspection helper on the owning Machine (live workers / live interactive Sessions / stopped primary Session with server / none) paralleling `steer_reader`.
- Wire `Write::TaskSteer` into `Write::remote()` (bounded 15 s SSH timeout) with pre-admission fallback to local save + synchronous tracker/carrier sync for remote-owned Tasks (no fallback on `OutcomeUnknown`).
- Share that exact path from CLI `lf steer <task>` (`commands/session.rs`) so CLI and served `task.steer` return identical `TaskSteerReceipt` JSON and CLI prints `delivery.sentence`.
- Update `tests/fixtures/dto/task_steer_receipt.json`, `server_schema.json`, `dto_fixtures.rs`, and `served_reads.rs`, and add cross-Machine steer tests (direct reachable, unreachable registered fallback, unregistered sibling sandbox, `local_only` when tracker offline, `OutcomeUnknown` no-double-write).

### Slice 3: Honest cross-Machine `lf task status` (`owner` + `execution_source`)
- Add `owner: Option<TaskOwner>` and `execution_source: TaskExecutionObservation` (`local | remote | remote_unreachable`) to `TaskDetail` (`rust/loopflow/src/lf/commands/task_row.rs`).
- Update `waves.rs` (`task_machine`, `remote_task_detail`, `print_remote_task_status`, `task_detail`, `snapshot_task_detail`) and `bin/lf.rs` (`TaskCommand::Status` text output):
  - Detect `ownership.owner_elsewhere(&local)` independently of `declared_machines`.
  - Query registered remote owners over SSH with a 15 s deadline; tag live replies `execution_source: Remote { machine }` with locally resolved Machine label.
  - On unregistered peer or SSH/server failure, return `execution_source: RemoteUnreachable { machine, reason }`, honest `task.reason`, and `task.next = None` (keeping terminal `done`/`abandoned` intact).
  - Print the remote-owner and reachability status clearly in non-JSON `lf task status`.
- Update `tests/fixtures/dto/task_detail.json`, `server_schema.json`, `dto_fixtures.rs`, `served_reads.rs`, and `machine_commands.rs` (reachable remote, unregistered sibling sandbox, unreachable registered peer).

## Done when

Derived from the Wave KRs (*"advance work without getting blocked by Loopflow's plumbing"*, *"don't have to track Sessions, remind agents to continue, or chase whether Tasks landed"*):

- An agent or person on a sibling sandbox with no SSH routes to peer sandboxes runs `lf steer <remote-task> "<direction>"`, sees an honest `Queued … in Linear for <machine> (not directly reachable; delivery unconfirmed until <machine> syncs)` receipt (and matching JSON `delivery.status: "queued"`), and the running worker or interactive Session on the owning sandbox receives that direction automatically within 30 s without manual intervention.
- An agent or person on a Machine that can SSH to the owning Machine runs `lf steer <remote-task> "<direction>"`, delivers it directly to the owning Machine's server in <1 s, and sees `Delivered … to <machine>; <live reader>` (falling back honestly to `queued`/`local_only` if SSH cannot connect).
- Running `lf task status <remote-task>` (text or `--json`) from an unreachable sibling sandbox or when a registered peer is offline explicitly names the owning Machine and reports `execution_source.kind == "remote_unreachable"` with `task.next == null`, never portraying a remote Task as locally idle or unstarted.
- Idle Machines with no owned open Tasks make zero background Linear requests, and active Machines stay within Linear's existing pacing and reserve limits.
- `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, Rust DTO/server/remote/steer suites, and Swift DTO/protocol tests pass.
