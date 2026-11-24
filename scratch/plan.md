# LOO-1083 · Server-routed remote Task workspace across CLI, Desktop, and Mobile

**Done when** a Task owned by another Machine (SSH host, pool Machine, or Modal rental) works end to end from CLI, Desktop, and a single-paired phone through the local/paired Loopflow server — with zero local checkout fallback and zero client-side SSH subprocess routing in Desktop:

- Desktop **Files**, **Changes**, **Diff**, **Save**, **Shell**, and **Session** all open against the owning Machine (even when a stale local checkout exists on the Mac). `MTG-421`-style failures (“The conversation belongs to a different Machine or checkout; nothing was opened.”) and local-Mac shell fallbacks are gone.
- Mobile exposes **Files** (directory tree + file content, including unchanged files) and **Shell** alongside **Changes** and **Session**, over the existing single pairing.
- `lf open` always presents on the calling Machine: `lf open --task <id> --tab` and `lf open --task <id> --session <sid> --tab` open a local tab attaching on the owning Machine, and `lf open --machine <label> --session <sid> --tab` (or `lf --machine <label> open ...`) attaches a remote-only Session locally.
- Unstarted Tasks show **Not started**, unreachable owners show an explicit error naming that Machine (never an empty diff or local shell), and moving a Task invalidates stale panes/watches/attachments immediately while retaining unsaved drafts at their original `(machine_id, checkout)`.

Derived from the Infrastructure Wave KRs: advancing and inspecting Tasks without Loopflow's plumbing blocking or misleading the person.

## Source of truth

- **Task ownership:** the append-only `task_ownership` log, resolved via `task_execution_route` (`rust/loopflow-execution/src/store/ownership.rs`, `machine/task.rs::remote_ownership_writer`). Ownership alone selects the execution Machine.
- **Checkout & execution state:** per-Machine `task_checkouts`, `agent_sessions`, `desktop_shells`, and held terminals (`transport::registry::Registry`) on the owning Machine. A stale `task_checkouts` row on a non-owning Machine is history only, never the active workspace.
- **Cross-Machine router:** the local/paired `lf server` (`rust/loopflow/src/server/`). Every native client (Desktop over Unix socket, Mobile over Tailscale WebSocket, CLI relays/reads) talks to that one server; the server talks to peer Machines over pooled `ssh … lf server connect` connections.

## Design and de-risked decisions

### 1. Pooled peer connections (`rust/loopflow-execution/src/machine/client.rs`, `rust/loopflow/src/server/peers.rs`)

- **Finding:** `Connection::open_on` (`machine/client.rs:189`) currently spawns a fresh `ssh … lf server connect` process and bakes one repository's `Sender` into `Carrier::Ssh(Machine, Box<Sender>)`. Reusing a connection across repositories or operations (e.g., `task.file` vs `task.run` with secrets) requires per-request `Sender`.
- **Mechanism:**
  - Add `Connection::open_peer(machine, until)` and `Connection::request_as(&mut self, operation, sender: Option<Sender>, request_id: Option<String>)` in `machine/client.rs`. `open_on` delegates to it for one-shot CLI callers.
  - Add `server/peers.rs::PeerPool` keyed by `(MachineId, route)` (route changes automatically discard old pipes). Pool up to 4 idle connections per peer so concurrent file/diff reads do not head-of-line block behind a slow diff or write; check out under a brief mutex (never held during I/O).
  - Idle eviction at 60s (matching OpenSSH `ControlPersist 60s`), pruned on checkout and on a 30s background timer in `server::run()`. Dropping a `Connection` drops `Pipe`, which kills and reaps its SSH child.
  - **Reconnect on error/EOF:**
    - For reads: if a reused (non-fresh) pooled connection fails with EOF, `BrokenPipe`, or `ConnectionReset`, drop it, open one fresh connection, and retry once. If the fresh connection fails, return `ErrorCode::Unavailable` naming `machine.name()` with structured data `{ resource: "machine", machine_id, machine_label, machine_name }`.
    - For writes: if connection setup fails, return `unavailable` naming the Machine. If bytes were already written on the pipe without a `request_id`, evict the pipe and return `outcome_unknown` (never blindly replay). When a write carries a `request_id`, the peer's rev 7+ deduplication cache (`server/answers.rs`) makes one reconnect retry safe.
  - Bare CLI commands outside `lf server` still use one-shot `Connection::open_on` so they leave no background SSH processes on exit.

### 2. Task-scoped read, write, and subscription routing (`commands/read.rs`, `commands/write.rs`, `server/subscriptions.rs`, `commands/waves.rs`)

- **Cross-Machine Task addressing (finding):** On Linear-backed repos (such as `manabot`), two Machines can hold different local `task_id` UUIDs for the same issue (`MTG-421`), while sharing `identifier` and `tracker_id`. Today `waves.rs:988` forwards `route.task_id`, which fails to resolve on a fresh sandbox unless the peer happens to share that UUID.
  - **Fix:** When forwarding a Task-scoped operation to a peer, resolve the Task's portable selector on the sender: prefer `tracker_id` (or `identifier` when `tracker_id` is absent) for Linear/Asana-backed Tasks, else `route.task_id` for trackerless plans. On the peer, `Origin::Machine` scopes `resolve_task_id` to the sender's `repository` checkout so identifiers are unambiguous.
- **Routed `Read` variants (when `Origin::Local` and `remote_ownership_writer(issue)` names another Machine):**
  - `task.status` (replaces `waves::remote_task_detail`), `task.get`, `task.detail`, `task.changes`, `task.diff`, `task.file`, `task.files`, plus `task.runs` and `session.list` when `task: Some(issue)` is set.
  - Unscoped `session.list { task: None }`, `plan.list`, `desktop.view.list`, `view.select`, and `wave.plan` stay local (inventories remain local; zero SSH fanout on list/board reads).
- **Routed `Write` variants:**
  - Add `Write::TaskSave` to `Write::remote()` (`launching: false` — sends no `forward.env` secrets and delivers no `forward.files`).
- **Routed subscriptions (`task.get`, `task.status`, and `task.files.watch`):**
  - **Finding:** Mobile (`TaskWorkspaceView.swift:64`) subscribes to `task.get`, and Desktop (`TaskWorkView.swift:501`) subscribes to `task.get` for remote Graph logs. Currently `Read::subscribe` only reads local SQLite. Meanwhile Desktop (`TaskFileDocument.swift:669`) spawns `lf --machine … task watch-files` via `RegistryQueryLocal.swift:30`.
  - **Decision (made for review, see `scratch/questions.md`):** Forward `task.get` and `task.status` subscriptions to the owning Machine over a dedicated peer `Subscribe` connection when `Origin::Local` and another Machine owns the Task, and add a subscribed `Read::TaskFilesWatch { issue, expected_owner }` backed by `ops/task/workspace.rs`'s existing `notify` watcher so Desktop remote file invalidation also travels over the single server protocol (deleting `RegistryQueryLocal`'s `watchFiles` and `subscribeRead` SSH subprocesses). Watch local `task_ownership` revisions beside the peer stream; when ownership changes, end the subscription with `conflict` (`actual_owner`) so clients invalidate stale panes and watches immediately.
- **Expected-owner CAS and absent/error typing:**
  - Add required-nullable `expected_owner: Option<MachineId>` (`#[serde(deserialize_with = "Option::deserialize")]`) to Task-scoped workspace operations (`task.status`, `task.get`, `task.detail`, `task.changes`, `task.diff`, `task.file`, `task.files`, `task.runs`, `task.files.watch`, `task.save`, `task.shell.*`, `session.attach`, `session.continue`, `session.start`).
  - Before forwarding (or reading locally), verify `task_execution_route` in local SQLite. If `expected_owner` is `Some(expected)` and `route.machine_id != expected`, immediately return `ErrorCode::Conflict` with `data: { "expected_owner": expected, "actual_owner": route.machine_id, "task_id": route.task_id }` without touching SSH.
  - **Finding:** `ops/task.rs:386` (`file_context`) returns an untyped `OpsError::Message("Task … is not checked out on this Machine")`, which `server/service.rs:1261` turns into `ErrorCode::Unavailable` — making an unstarted Task indistinguishable from an unreachable Machine!
  - **Fix:** Return typed `engine::Error::new(ErrorCode::NotFound, format!("Task {issue} is not started")).with(json!({ "reason": "not_started", "task_id": task_id }))` when the Task exists on its owner but has no `task_checkouts` row.
- **Protocol revision:** Bump server protocol `REVISION` and `MIN_REVISION` from 9/8 to 10/10 (matching Loopflow's no-backcompat wire rule in `server/README.md`). Regenerate `tests/fixtures/dto/server_schema.json`, update fixtures under `rust/loopflow-execution/tests/fixtures/dto/server/`, and update `swift/Loopflow/Services/ServerProtocol.swift` and `ServerProtocolTests.swift`.

### 3. Terminal ticket bridging and Task shells (`server/service.rs`, `server/ticket.rs`, `server/remote.rs`, `machine/wire.rs`, `desktop/shells.rs`)

- **Finding 1 (Session ownership discovery):** A Session created natively on a Modal sandbox has no row in the laptop's `agent_sessions` table. If `session.attach` / `session.continue` carry only `session: LfSessionId`, the laptop server cannot tell which peer Machine owns it.
  - **Fix:** Add required-nullable `task: Option<String>`, `machine: Option<MachineId>`, and `expected_owner: Option<MachineId>` to `SessionAttach`, `SessionContinue`, and `SessionStart`.
  - Routing resolution when `Origin::Local`:
    1. If `task` is `Some(issue)`: resolve `task_execution_route` (enforcing `expected_owner` -> `conflict`). If another Machine owns it, route there.
    2. Else if `machine` is `Some(m)` and `m != local_machine`: route to `m`.
    3. Else check local `conversation_task(&store, &session)` -> `task_execution_route` and `session_moved_to(&store, &session)`. If another Machine owns it, route there.
    4. Else run locally.
- **Finding 2 (Task shells over the server):** Today `desktop/shells.rs` runs via `lf --machine <id> --repo-id <r> desktop shell <task> --checkout <path> --id <tab>` over SSH, and `session.attach` requires a `LfSessionId`, so Mobile has no Shell access and Desktop shells bypass the server.
  - **Fix:** Add three `ServerOperation` variants mirroring `session.continue` + `session.attach`:
    - `task.shell.start { issue, shell_id, rows, cols, bootstrap: Option<String>, command: Vec<String>, expected_owner: Option<MachineId> }` -> `SessionStarted { terminal_id }`. Idempotent by `shell_id` via `desktop_shells`; checks `expected_owner`; forwards to owner when remote; returns `not_found { reason: "not_started" }` if no checkout exists on the owner; starts `$SHELL` (with optional Ghostty `bootstrap` JSON for Desktop, `null` for Mobile) in the owner's checkout inside `Registry`.
    - `task.shell.attach { issue, shell_id, terminal_id: Option<TerminalId>, expected_owner: Option<MachineId> }` -> `SessionAttached { ticket, terminal_id, expires_in_ms }`. Never starts a shell; attaches to existing held terminal or returns `not_found` (ended) / `unavailable` (holder waiting).
    - `task.shell.close { issue, shell_id, expected_owner: Option<MachineId> }` -> `{}` (closes shell on owner).
  - `lf desktop shell` (invoked by Desktop's Ghostty terminal pane) and `lf session attach` route through the local server's `task.shell.*` / `session.attach` + ticket bridge rather than spawning `ssh` directly!
- **Async terminal ticket bridging (`server/ticket.rs`, `server/service.rs`, `server/remote.rs`):**
  - Add `Purpose::RemoteTerminal { machine: Machine, remote_ticket: String, terminal_id: TerminalId }` to `Tickets`. Single-use, admission-bound (`Admission::Local` or `Admission::Remote { node, user }`), spent on any foreign attempt, 60s wall-clock lifetime.
  - When `session.attach` or `task.shell.attach` routes to peer `M`, the local server calls `M`'s `session.attach` / `task.shell.attach` over `PeerPool`, receives `remote_ticket`, issues a local `Purpose::RemoteTerminal` ticket bound to the caller's `Admission`, and returns `SessionAttached { ticket: local_ticket, terminal_id, expires_in_ms }`.
  - On redemption (`service::ticketed`), spawn async `tokio::process::Command::new("ssh") … lf server connect` (`kill_on_drop(true)`), write `{"type":"attach","ticket":"<remote_ticket>"}\n`, read the single-line JSON answer (`bridged` or `error` — forwarding any peer error back to the client), send `ServerMessage::Bridged { terminal_id }`, and return a unified `TerminalStream` (`Local(UnixStream)` | `Remote(RemoteTerminal)` implementing `AsyncRead + AsyncWrite + Unpin + Send`) to BOTH `service::bridge` (Desktop Unix socket `copy_bidirectional`) and `remote::bridge` (Mobile WebSocket 64 KiB binary frames).
  - Preserves viewer protocol framing, resize/input, heartbeats, and `0x07` close frames (`replaced`, `finished`, `retired`) verbatim.

### 4. Ownership-driven `RoadmapTask` / `TaskReferenceSnapshot` (`rust/loopflow/src/lf/commands/waves.rs`)

- **Finding:** `waves.rs:2697` (`task_reference`) unconditionally stamps `local_machine` onto `TaskWorktreeSnapshot` whenever a local `task_checkouts` row exists — even when `task_ownership` / `task_execution_route` shows another Machine owns the Task. That caused Desktop to treat the stale Mac checkout as active, open a local Mac shell, and reject the remote Session with `"The conversation belongs to a different Machine or checkout; nothing was opened."` (`SessionsView.swift:719`).
- **Fix:** Pass the Task's resolved ownership (`facts.owners.get(&task.id)` / `task_execution_route`) into `task_reference`:
  - When the Task is owned by another Machine (`owner_machine != local_machine`): **never expose the stale local `task_checkouts` path as `reference.workspace`**. Instead, `reference.workspace` either reflects the owning Machine (`machine_id: Some(owner_machine)`, `machine_label`, `slug: task.workspace_slug`, `branch: None`, `base_commit: ""`, `worktree: ""`, `local_exists: Some(false)`) OR `RoadmapTask` exposes the owner cleanly via `runtime.owner` + `reference.workspace = None` on non-owners while `task.get` from the owner supplies the remote `reference.workspace`!
  - **Decision (made for review, see `scratch/questions.md`):** On a non-owning Machine, `task_reference` sets `reference.workspace = None` (never fabricating a remote path or exposing a stale local worktree), while `runtime.owner` (`TaskOwner { person, ..., machine: MachineId }`) + `runtime.machine_label` (or lookup in `machine.list`) identifies the owning Machine. When Desktop or Mobile opens the Task, routed `task.get` returns the **owning Machine's** `DesktopTaskPane`, whose `row.reference.workspace` carries the authoritative `(machine_id, machine_label, worktree, branch, base_commit)`! Update `waves.rs::task_reference_names_only_this_machines_checkout` to also assert that a stale local `task_checkouts` row is hidden once another Machine claims the Task.
  - Also fix `ops/task/snapshot.rs:146` (`read_task_snapshot`) with the exact same rule so local `task.status` never reports a stale local checkout as active when owned elsewhere.

### 5. Desktop (`swift/LoopflowMac/WorkModel.swift`, `Views/SessionsView.swift`, `TaskFileDocument.swift`, `Views/TaskWorkView.swift`, `Views/WorkspacePanesView.swift`, `swift/Loopflow/Services/RegistryQuery.swift`)

- **Delete client-side SSH routing:**
  - Remove `RegistryQuery.onMachine`, `RegistryQuery.pinTaskLocation`, `RegistryQuery.remoteMachine`, `RegistryQuery.taskLocation` (from Task workspace flows), `SessionsStore.executionLocations`, `LinkedSession.location`, and `RegistryQueryLocal`'s `watchFiles` / `subscribeRead` SSH subprocess runners.
  - Every Task read (`task.get`, `task.detail`, `task.changes`, `task.diff`, `task.file`, `task.files`, `task.files.watch`), write (`task.save`, `session.resume`), and terminal (`session.attach`, `session.continue`, `task.shell.*`) goes through `RegistryQueryLocal.shared` -> `ServerClient` -> local `lf server`.
- **Owner resolution & invalidation:**
  - Before selecting retained panes, Sessions, or shells (cold open, restoration, prefetch, tab creation, `openLinkedTask`), resolve the Task's current owner from `RoadmapTask.runtime?.owner?.machine` (falling back to `localMachineId` when owned locally).
  - Pass `expectedOwner: workspace.ownerMachineId` on all Task workspace reads, saves, watches, and shell/session attachments.
  - Keep the placement mismatch guard (`SessionsView.swift:716`) — now comparing against the resolved owner's `(machineId, worktree)` from the server-routed `task.get` / `session.resume` response rather than a stale local checkout.
  - When ownership changes (observed via `plan.list` reading update OR an immediate `conflict` response carrying `actual_owner`):
    - Cancel active file watches and detach/invalidate stale terminal panes & cached `TaskFilesStore` bound to the previous owner.
    - Retain unsaved drafts under their original `(machine_id, checkout)` in `unavailableDrafts` / `WorkPaneLayout.drafts` (never redirecting or overwriting them onto the new owner's checkout).
    - Present a clear moved notice / reopen action pointing to the new owner.
  - When a Task is unstarted (`not_found` with `reason == "not_started"`), render **Not started** (no empty diff, no local shell). When the owner is unreachable (`unavailable` with `resource == "machine"`), render an explicit error naming the Machine with **Retry**.
  - For Task shells: `SessionsWorkspace.shellAttachment` runs local `lf desktop shell <issue> --id <tab.id> [--expected-owner <machineId>] [--create --bootstrap <json>]` (no `--machine`, no `--checkout` local path assumptions), and `closePane` calls `task.shell.close` through `ServerClient` instead of spawning `lf --machine ... desktop shell --close`.
  - For Task Sessions: `Write::SessionResume` returns a `SessionRecord` whose `open_argv` attaches via the local server (`lf session attach --session <id> --task <task> --expected-owner <machineId>`), eliminating raw `ssh` argv from `prepared_remotely` for Desktop.

### 6. Mobile (`swift/LoopflowiOS/TaskWorkspaceView.swift`, `RemoteMobileClient.swift`, `SessionLink.swift`)

- **Single pairing, four viewers:** Keep the existing single Tailscale pairing to the user's primary Machine server. All four viewers (**Session**, **Files**, **Changes**, **Shell**) talk to that paired server, which routes to the Task's owning Machine transparently.
- **Finding (Mobile primary Session discovery on remote Tasks):** `TaskWorkspaceView.swift:87-106` calls `client.task(id:directory:)` (`desktop.view.get`) on the paired laptop, which reads local SQLite and returns `primarySession == nil` for a remote-Machine Task.
  - **Fix:** Derive the remote Task's primary Session directly from the routed `task.get` subscription frame (`reading.work.sessions`, selecting the primary/open interactive Session, or expose `primary_session` on `DesktopTaskPane` / `TaskOverview`!) AND pass `task: task.id` + `expectedOwner` on `session.attach` and `session.continue` in `RemoteMobileClient` so the paired server routes them to the owning Machine!
- **Files viewer (tree + unchanged files):**
  - Add `TaskFilesTreeView` inside `TaskWorkspaceView` (accessible from the Task workspace segmented control / toolbar alongside Session, Changes, and Shell).
  - Calls `client.query.taskFiles(issue: task.id, path: dir, ...)` to browse directories (with pagination via `next_cursor`) and opens any file (including **unchanged** files) in the existing `TaskFileView` (`client.query.taskFile(issue: task.id, path: path)`).
- **Shell viewer:**
  - Add `TaskShellView` + `ShellLink` (sharing `RemoteAttachment`, `SessionTerminal`, `GhosttySurface`, and the reply/special-keys bar with `SessionView`) inside `TaskWorkspaceView`.
  - Uses a stable per-Task mobile shell ID (e.g. `mobile-<task.id>`), calling `task.shell.attach` first and `task.shell.start` on explicit Start / first open (`bootstrap: nil`, `command: []`, `rows`/`cols` from `TerminalGeometry`), then connecting over `/v1/ticket` WebSocket with the bridged ticket!
- **Absent and error states on Mobile:**
  - Unstarted Task (`not_found { reason: "not_started" }` or `reading.checkout == nil` on an unstarted Task): show **Not started** on Files, Changes, and Shell (never `"No changes yet"` or a local shell).
  - Unreachable owner (`unavailable` with `resource == "machine"`): show explicit error naming the unreachable Machine + **Try again** (never an empty diff or local shell).
  - Task moved (`conflict` with `actual_owner`): show moved state + **Refresh** to reload from the new owner.

### 7. CLI `lf open` (`rust/loopflow/src/lf/commands/open.rs`, `commands/open/conversation.rs`, `lf/navigation.rs`, `bin/lf.rs`)

- **Finding 1 (`--machine` hijacked `lf open`):** `navigation.rs:137` (`machine_invocation`) strips top-level `--machine <label>` and forwards ANY command to SSH unless the subcommand itself declares `--machine` AND it appears after the subcommand! Look at `navigation.rs:154-155`:
  `let is_transport = matches!(name, "--machine" | "--forward-agent") && (path.is_empty() || flag(current, name).is_none());`
  So `lf --machine modal-1 open --session sid --tab` currently SSHes into `modal-1` and runs `lf open` on a headless Linux container (which fails!).
  - **Fix:**
    1. Add `#[arg(long, id = "open_machine")] pub machine: Option<String>` to `OpenSelection` in `commands/open.rs` (distinct Clap `id` per `AGENTS.md` so it never leaks into root `Cli.machine`).
    2. In `bin/lf.rs:1733` (or `navigation::machine_invocation`), special-case `Commands::Open`: `lf open` **never** forwards execution onto the remote Machine. When `--machine <label>` is given in *either* position (`lf open --machine <label> ...` or `lf --machine <label> open ...`), normalize it into `OpenSelection.machine` and run `lf open` locally! (Reject `--forward-agent` with `open` or forward it into the local attach command if needed.)
- **Finding 2 (`resume_argv` dropped `--task` when `--session` was set):**
  Look at `open.rs:191-198`:
  ```rust
  match (&self.session, &self.url.target) {
      (Some(session), _) => argv.push(session.clone()),
      (None, WorkTarget::Task(id)) => argv.extend(["--task".into(), id.clone()]),
      ...
  ```
  When `lf open --task LOO-1083 --session session_abc --tab` runs on a remote Task, `resume_argv` emitted `lf session resume session_abc` WITHOUT `--task LOO-1083` or `--machine` — so the spawned tab looked up `session_abc` locally (not found!) or attached to a stale local checkout!
  - **Fix:**
    - Keep `--task <id>` (and `--repo-id <repository_id>` when remote) alongside `<session>` in `resume_argv` when `self.task` / `WorkTarget::Task` is set, AND when `self.execution_machine` (or explicit `--machine <label>`) names another Machine, pass `--machine <machine>` + `--session` / `<session>` (or `lf --machine <owner> [--repo-id <repo>] session resume <sid> [--task <id>]`) so the local tab ALWAYS attaches on the owning Machine — never falling back to a stale local checkout!
    - For `lf open --machine <label> --session <sid> --tab` (remote-only Session with no local Task):
      - Resolve `<label>` against registered Machines via `find_registered_machine(label)` (fail with retryable error if unknown/unreachable).
      - Skip local `context::explain_in` lookup for `<sid>` (since `<sid>` does not exist in local SQLite!).
      - Build `Resolved` with `execution_machine = Some(machine.id)`, `session = Some(sid)`, `label = format!("{label} · {sid}")`, and `resume_argv` attaching on `<machine>` locally (keeping `Attachment` receipt and retry semantics in `open/conversation.rs`).
      - Verify `lf open --task <id> --tab` with a stale local checkout attaches on `execution_machine` (never local!) because `describe()` already sets `execution_machine = Some(owner)` from `task_location::explain` -> `task_execution_route`. Also fix `Resolved::cwd()` / `conversation_view()` so a stale local `session_cwd` or local `agent_sessions` row is never used when `execution_machine.is_some()`.

## Affected surfaces and consumers

- **Server protocol (`machine/wire.rs`, `server/wire.rs`, `server/schema.rs`, `tests/fixtures/dto/server_schema.json`, `rust/loopflow-execution/tests/fixtures/dto/server/*.json`):** Revision 10; `expected_owner` on Task workspace operations; `task`/`machine`/`expected_owner` on `session.attach`/`continue`/`start`; `task.shell.start`/`attach`/`close`; `task.files.watch` subscription; structured `not_started` (`not_found`), `conflict` (`actual_owner`), and `machine` (`unavailable`) errors.
- **Rust CLI & server (`loopflow`, `loopflow-execution`):** `server/peers.rs`, `server/service.rs`, `server/ticket.rs`, `server/remote.rs`, `server/subscriptions.rs`, `machine/client.rs`, `lf/commands/read.rs`, `lf/commands/write.rs`, `lf/commands/waves.rs`, `lf/commands/open.rs`, `lf/commands/open/conversation.rs`, `lf/commands/desktop.rs`, `lf/navigation.rs`, `bin/lf.rs`, `ops/task.rs`, `ops/task/snapshot.rs`, `desktop/shells.rs`.
- **Shared Swift (`swift/Loopflow`):** `Services/ServerProtocol.swift`, `Services/RegistryQuery.swift`, `Models/WaveWorkMap.swift`, `Models/TaskWork.swift`.
- **Desktop macOS (`swift/LoopflowMac`):** `WorkModel.swift`, `Views/SessionsView.swift`, `Views/WorkspacePanesView.swift`, `Views/TaskWorkView.swift`, `Views/TaskFilesView.swift`, `Views/TaskChangesView.swift`, `TaskFileDocument.swift`, `Services/RegistryQueryLocal.swift`, `Services/ServerClient.swift`.
- **Mobile iOS (`swift/LoopflowiOS`):** `TaskWorkspaceView.swift`, `RemoteMobileClient.swift`, `SessionLink.swift`, `SessionView.swift` (shared terminal chrome).
- **Docs:** `docs/surfaces.md`, `docs/workspaces.md`, `docs/server.md`, `docs/ops.md`, `docs/lf-reference.md`, `rust/loopflow/src/server/README.md`, `docs/dev/machines.md` (updating the client-side SSH note on line 247-252 and server routing description).

## Absent and error states

| State | Wire `code` & `data` | CLI (`lf open` / `lf task ...`) | Desktop | Mobile |
| --- | --- | --- | --- | --- |
| Task unstarted (no checkout on owner) | `not_found` `{ "reason": "not_started", "task_id": "…" }` | Explicit message: Task is not started | **Not started** in Files, Changes, Shell (never empty diff / local shell) | **Not started** in Files, Changes, Shell |
| Owner Machine unreachable / SSH failed | `unavailable` `{ "resource": "machine", "machine_id": "…", "machine_label": "…", "machine_name": "…" }` | Retryable failure naming Machine + `ssh <route> lf server status` | Explicit error naming Machine + **Retry** | Explicit error naming Machine + **Try again** |
| Task moved (`expected_owner` mismatch) | `conflict` `{ "expected_owner": "…", "actual_owner": "…", "task_id": "…" }` | Immediate refusal naming new owner | Invalidate panes/watches/attachments; retain unsaved drafts at original `(machine_id, checkout)`; show Reopen | Show moved notice + **Refresh** to reload from new owner |
| Session/shell ended on owner | `not_found` `{ "terminal_id": "…", "exit_code": … }` | Reports ended; never auto-restarts on attach | Ended screen + explicit Start / New Shell | Ended state + Start / New Shell |
| Holder slow to answer after restart | `unavailable` `{ "resource": "holder", "terminal_id": "…", "retry": true }` | Retries / reports waiting | Retries without starting a duplicate | Retries without starting a duplicate |

## Operational boundary

- **Peer connection pool:** max 4 idle connections per `(MachineId, route)`; 60s idle eviction; 10s handshake timeout; 120s request timeout (existing `client.rs` bounds). Warm remote `task.file`/`task.files`/`task.diff` round-trips pay zero SSH handshake cost.
- **Payload & ticket limits:** control messages bounded at 1 MiB (`MAX_MESSAGE`); bulk downloads/uploads (large diffs, drafts, 1 MB files) bounded at 32 MiB (`MAX_BULK`) over single-use tickets; terminal tickets single-use, admission-bound, 60s wall-clock expiry; remote terminal bridge copies 64 KiB WebSocket binary frames with 5s write grace.
- **No list/board fanout:** `plan.list`, `desktop.view.list`, `view.select`, and `wave.plan` read local SQLite only; peer connections open only when a Task's workspace is opened or queried directly.
- **No silent local fallback:** if the owning Machine is unreachable or a Task has moved, no operation ever falls back to a local checkout, local shell, or local Session.

## Exclusions

- Modal sandbox creation, burst sizing, Linux package upload, Wave adoption, analytics collection, harness fallback, and `lf machine forward` (LOO-1081).
- Cross-sandbox / cross-Machine steering (`task.steer` / `session.steer` delivery across Machines; LOO-1084).
- Multi-Machine pairing on Mobile (phone stays paired to one Machine server).
- Changing `lf task location --peers` CLI diagnostics (keep the `lf task location` CLI command and `TaskLocationObservation` DTO if still used by CLI diagnostics; only remove Desktop's `onMachine` / `pinTaskLocation` Task-view routing stack!).

## What goes

- `swift/Loopflow/Services/RegistryQuery.swift`: `onMachine(_:repository:)`, `pinTaskLocation(to:)`, `remoteMachine`, `observeFiles` / `watchTaskFiles` CLI-subprocess plumbing, and `observeRead` CLI-subprocess plumbing for remote Tasks.
- `swift/LoopflowMac/Services/RegistryQueryLocal.swift`: `watchFiles` and `subscribeRead` SSH subprocess runners (`LocalLineObservation` for remote Tasks, if no other caller remains — verify before deleting).
- `swift/LoopflowMac/Views/SessionsView.swift`: `SessionsStore.executionLocations`, `retainLocation`, `query(for:)`, `onMachine`/`pinTaskLocation` branches in `files(for:task:query:)`, `openWarmConversation`, `loadPaneIntent`, and raw `--machine ... desktop shell` CLI invocation in `shellCommand`.
- `swift/LoopflowMac/WorkModel.swift`: `LinkedSession.location`, `onMachine` / `taskLocation` double-SSH probe in `prepareLinkedTask`.
- `swift/LoopflowMac/Views/TaskWorkView.swift`: `onMachine` / `pinTaskLocation` branch in `GraphProcessLog`.
- `swift/LoopflowMac/TaskFileDocument.swift`: `query.remoteMachine` branches and `observeRemoteFiles` subprocess watcher (replaced by server-routed `task.files.watch` subscription and `ownerMachineId`).
- `rust/loopflow/src/lf/commands/waves.rs`: bespoke one-shot `remote_task_detail` / `task_machine` (replaced by the unified pooled Task router in `commands/read.rs`).
- `rust/loopflow/src/lf/commands/ssh.rs`: `prepared_remotely` wrapping `open_argv` in raw `ssh` for Desktop (replaced by local server-routed `lf session attach` while keeping CLI `lf --machine <m> resume` interactive SSH working if invoked directly).
- Obsolete Swift tests asserting `RegistryQuery.onMachine` / `pinTaskLocation` subprocess args (replaced with server-routed tests in `RemoteTaskWorkspaceTests` and updated `TaskFilesTests` / `RegistryQueryTests`).

**Must survive:**
- Local Task Files, Changes, Diff, Save, Shell, Session, and FSEvents watching on the local Machine.
- Unsaved draft preservation across reconnects and Task moves, keyed by `(machine_id, checkout)`.
- `lf task location --peers --json` diagnostic CLI command and `tests/fixtures/dto/task_locations.json`.
- `lf --machine <label> <cmd>` SSH transport for all non-`open` commands (and `lf --machine <label> resume` direct CLI transport).
- Mobile's existing single-pairing Tailscale admission, Task creation journal, and Session terminal controls (`Continue here`, reply bar, special keys).

## Slices

Build thinly end to end in four slices; each slice runs and proves itself before moving on, and deletes what it replaces as soon as it works:

1. **Slice 1 (de-risks the hardest core): Server peer pool, routed Task reads/writes/subscriptions, and bridged terminal tickets (`Session` + `TaskShell`).**
   - Build `PeerPool` (`server/peers.rs`, `machine/client.rs`), route `task.status`/`get`/`detail`/`changes`/`diff`/`file`/`files`/`runs`/`files.watch` + `task.save` + `session.attach`/`continue`/`start` + `task.shell.start`/`attach`/`close` with `expected_owner` (`conflict`), `not_started`, and `unavailable` typing, and implement async `Purpose::RemoteTerminal` bridging for both Unix socket and WebSocket (`service::bridge` + `remote::bridge`).
   - Fix `waves.rs::task_reference` and `ops/task/snapshot.rs` so stale local checkouts never masquerade as active when another Machine owns the Task.
   - Bump protocol to revision 10, regenerate `tests/fixtures/dto/server_schema.json`, update server fixtures and `ServerProtocol.swift` / `ServerProtocolTests.swift`.
   - **Proves:** A two-Machine Rust integration test (`machine_commands::server_routed_task_workspace`) with a stale local checkout (distinct file bytes) reads remote unchanged files, uncommitted diff, saves a remote file, starts + attaches a remote Task shell (`hostname` & `pwd`) and remote Session through the local server's Unix socket AND WebSocket carrier, reuses the pooled peer connection across reads, returns `conflict` on Task move, `not_found/not_started` before checkout, and `unavailable` naming the peer when unreachable.

2. **Slice 2: CLI `lf open` always local + remote Task/Session attachment.**
   - Add `--machine` to `OpenSelection` (distinct Clap id), exempt `open` from remote SSH execution in `navigation.rs` / `bin/lf.rs` for both flag positions (`lf open --machine <label> ...` and `lf --machine <label> open ...`).
   - Fix `open.rs::resume_argv` keeping `--task` + owner Machine/repo when `--session` is set, ignore stale local checkout when `execution_machine.is_some()`, and support remote-only Sessions (`lf open --machine <label> --session <sid> --tab`) with `Attachment` receipt and retryable unavailable errors.
   - **Proves:** Integration test opening a remote Task (`--task <id> --tab` with a stale local checkout), a specific Session on a remote Task (`--task <id> --session <sid> --tab`), and a remote-only Session (`--machine <label> --session <sid> --tab` in both flag positions), all presenting locally and attaching on the peer.

3. **Slice 3: Desktop migration + deletion of `RegistryQuery.onMachine` / `pinTaskLocation`.**
   - Switch Desktop Files, Changes, Diff, Save, Watch, Graph log, Shell, and Session onto local `ServerClient` routed operations with `expectedOwner`.
   - Resolve Task owner before selecting retained panes/Sessions/shells; invalidate stale panes/watches/attachments on ownership change while keeping unsaved drafts at `(machine_id, checkout)`.
   - Delete `RegistryQuery.onMachine`, `pinTaskLocation`, `remoteMachine`, `executionLocations`, and `RegistryQueryLocal`'s remote SSH subprocess runners; update `lf desktop shell` and `prepared_remotely` to attach through the local server ticket bridge.
   - **Proves:** `RemoteTaskWorkspaceTests` on macOS exercising all four viewers (Files unchanged content, Changes uncommitted hunk + Save, Shell `hostname`/`pwd`, Session input acknowledgement) against distinguishable local vs remote server responses, plus disconnect/reopen, unstarted, unreachable, and Task-move draft preservation.

4. **Slice 4: Mobile Files & Shell viewers + Docs + Gate.**
   - Expose Files (directory tree + unchanged file content via `task.files` / `task.file`) and Shell (via `task.shell.start` / `task.shell.attach` + `/v1/ticket` WebSocket) alongside Changes and Session in `TaskWorkspaceView.swift` / `RemoteMobileClient.swift`.
   - Derive remote Task primary Session from routed `task.get` and pass `task` + `expectedOwner` on `session.attach` / `session.continue`. Show **Not started** and explicit unreachable-Machine errors.
   - Update user and dev docs (`docs/surfaces.md`, `docs/workspaces.md`, `docs/server.md`, `docs/ops.md`, `docs/lf-reference.md`, `rust/loopflow/src/server/README.md`, `docs/dev/machines.md`).
   - **Proves:** `LoopflowiOSTests/RemoteTaskWorkspaceTests` exercising all four viewers, unstarted, unreachable, and Task-move handling over one paired server connection.

## Acceptance check at gate

Headless commands across all layers (two isolated Machines with a stale local checkout containing different file bytes, a live remote checkout with an unchanged remote-only file and uncommitted hunk, a running remote Session, and a remote shell):

```sh
cargo test -p loopflow --test integration machine_commands::server_routed_task_workspace
cargo test -p loopflow --test integration open_remote_workspace
scripts/test_desktop.sh --filter RemoteTaskWorkspaceTests
uv run python scripts/test_ios.py -only-testing:LoopflowiOSTests/RemoteTaskWorkspaceTests
```

Expected results:
- `server_routed_task_workspace` passes: pooled peer reuse (verified by peer client/connection count), remote unchanged file + uncommitted diff + remote save, bridged Session and Shell tickets over both Unix socket and WebSocket, `not_started` vs `unavailable` (naming Machine) vs `conflict` on move, and stale local checkout never exposed in `RoadmapTask`.
- `open_remote_workspace` passes: `lf open --task <id> --tab`, `lf open --task <id> --session <sid> --tab`, and `lf open --machine <label> --session <sid> --tab` (both flag orders) present locally and attach on the peer despite a stale local checkout.
- Desktop and Mobile `RemoteTaskWorkspaceTests` pass: all four viewers (Files unchanged file, Changes uncommitted hunk, Shell `hostname` + `pwd`, Session input ack) succeed through the single server protocol, show **Not started** before checkout and explicit unreachable-Machine errors on outage, and invalidate stale panes while retaining unsaved drafts on Task move.

## Demo

With a Task running on a remote Machine (e.g. a Modal rental or SSH peer) and a stale local checkout on the laptop:
1. Run `lf open --task <id> --tab` — opens a local tab attached to the remote Session (not the stale local checkout).
2. Run `lf open --machine <label> --session <remote-sid> --tab` — opens a local tab attached to the remote-only Session.
3. Open the Task in **Loopflow Desktop** — **Files** shows the remote working copy and saves edits to the remote, **Changes** shows the remote uncommitted diff, **Shell** runs `hostname && pwd` on the remote Machine, and **Session** attaches cleanly with live input.
4. Open the same Task on the **paired iPhone** — switch between **Session**, **Files** (browse tree & read an unchanged remote file), **Changes**, and **Shell** (`hostname && pwd`), all over the existing single pairing.
