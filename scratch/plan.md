# LOO-1083 · Server-routed remote Task workspace across CLI, Desktop, and Mobile

**Done when** a Task owned by another Machine (SSH host, pool Machine, or Modal rental) works end to end from CLI, Desktop, and a single-paired phone through the local/paired Loopflow server — with zero local checkout fallback and zero client-side SSH subprocess routing in Desktop:

- Desktop **Files**, **Changes**, **Diff**, **Save**, **Shell**, and **Session** all open against the owning Machine (even when a stale local checkout exists on the Mac). `MTG-421`-style failures (“The conversation belongs to a different Machine or checkout; nothing was opened.”) and local-Mac shell fallbacks are gone.
- Mobile exposes **Files** (directory tree + file content, including unchanged files) and **Shell** alongside **Changes** and **Session**, over the existing single pairing.
- `lf open` always presents on the calling Machine: `lf open --task <id> --tab` and `lf open --task <id> --session <sid> --tab` open a local tab attaching on the owning Machine, and `lf open --machine <label> --session <sid> --tab` (or `lf --machine <label> open ...`) attaches a remote-only Session locally.
- Unstarted Tasks show **Not started**, unreachable owners show an explicit error naming that Machine (never an empty diff or local shell), and a Task that moves is followed: open Task views re-read from the new Machine on their own, terminals already running on the old Machine stay labelled with it, and an unsaved draft saves to wherever the Task is now only if the file there is still at the draft's base revision.

Derived from the Infrastructure Wave KRs: advancing and inspecting Tasks without Loopflow's plumbing blocking or misleading the person.

## The model (concept review, 2026-11-23)

**The Task is the address.** A person opens a Task; the Task is on one Machine; every view shows that Machine's checkout. No client holds a second opinion about where the Task is, so nothing needs comparing.

- Clients name the Task (and, for a Session with no Task here, its Machine). Only the local `lf server` resolves ownership and routes. There is no `expected_owner` field, no `conflict` answer, and no client-side "location".
- A subscription is to the Task, not to a Machine: when local `task_ownership` advances, the server drops the peer stream and opens one to the new owner; the next reading carries the new `reference.workspace.machine_id`, and `task.files.watch` emits one whole-tree invalidation. The client notices the Machine changed from the reading itself.
- A draft belongs to `(Task, path, base revision)`. `task.save` already refuses when the file's content revision differs (`ops/task/file_save.rs`), which is the only guard a write needs: identical content on the new owner saves there; different content is a conflict that keeps the draft.
- A terminal is a process on one Machine. Moving the Task neither kills nor retargets a Shell or Session already running; its pane names its Machine and ends on its own `0x07` close.

Passed over: the earlier `expected_owner` compare-and-swap on 14 operations with drafts held at `(machine_id, checkout)`. It made the client a second router, turned a move into an error on every open pane, and kept the guard (`SessionsView.swift:716`) that produced MTG-421's "The conversation belongs to a different Machine or checkout; nothing was opened."

## Source of truth

- **Task ownership:** the append-only `task_ownership` log, resolved via `task_execution_route` (`rust/loopflow-execution/src/store/ownership.rs`, `machine/task.rs::remote_task`). Ownership alone selects the execution Machine.
- **Checkout & execution state:** per-Machine `task_checkouts`, `agent_sessions`, `desktop_shells`, and held terminals (`transport::registry::Registry`) on the owning Machine. A stale `task_checkouts` row on a non-owning Machine is history only, never the active workspace.
- **Cross-Machine router:** the local/paired `lf server` (`rust/loopflow/src/server/`). Every native client (Desktop over Unix socket, Mobile over Tailscale WebSocket, CLI relays/reads) talks to that one server; the server talks to peer Machines over pooled `ssh … lf server connect` connections.

## Design and de-risked decisions

### 1. Pooled peer connections (`rust/loopflow-execution/src/machine/client.rs`, `rust/loopflow/src/server/peers.rs`)

- **Before pooling:** `Connection::open_on` spawned a fresh `ssh … lf server connect` process and baked one repository's `Sender` into `Carrier::Ssh(Machine, Box<Sender>)`. Reusing a connection across repositories or operations (e.g., `task.file` vs `task.run` with secrets) requires per-request `Sender`.
- **Mechanism:**
  - `Connection::open_peer` opens the SSH control connection. `Connection::request_as` supplies the sender, request ID and disconnect outcome per request. One-shot CLI callers use `open_on` with a sender for the connection’s lifetime.
  - `server/peers.rs::PeerPool` holds up to four idle connections per Machine and route. A new route retires the old pipes. Concurrent readers can open separate connections. SSH I/O and child cleanup run outside the pool locks.
  - Idle eviction at 60s (matching OpenSSH `ControlPersist 60s`), pruned on checkout and on a 30s background timer in `server::run()`. Dropping a `Connection` drops `Pipe`, which kills and reaps its SSH child.
  - **Reconnect on error/EOF:**
    - For reads: if a reused (non-fresh) pooled connection fails with EOF, `BrokenPipe`, or `ConnectionReset`, drop it, open one fresh connection, and retry once. If the fresh connection fails, return `ErrorCode::Unavailable` naming `machine.name()` with structured data `{ resource: "machine", machine_id, machine_label, machine_name }`.
    - For writes: if connection setup fails, return `unavailable` naming the Machine. If bytes were already written on the pipe without a `request_id`, evict the pipe and return `outcome_unknown` (never blindly replay). When a write carries a `request_id`, the peer's rev 7+ deduplication cache (`server/answers.rs`) makes one reconnect retry safe.
  - Bare CLI commands outside `lf server` still use one-shot `Connection::open_on` so they leave no background SSH processes on exit.

### 2. Task-scoped read, write, and subscription routing (`commands/read.rs`, `commands/write.rs`, `server/subscriptions.rs`, `commands/waves.rs`)

- **Cross-Machine Task addressing (finding):** On Linear-backed repos (such as `manabot`), two Machines can hold different local `task_id` UUIDs for the same issue (`MTG-421`), while sharing `identifier` and `tracker_id`. Today `waves.rs:988` forwards `route.task_id`, which fails to resolve on a fresh sandbox unless the peer happens to share that UUID.
  - **Fix:** When forwarding a Task-scoped operation to a peer, resolve the Task's portable selector on the sender: prefer `tracker_id` (or `identifier` when `tracker_id` is absent) for Linear/Asana-backed Tasks, else `route.task_id` for trackerless plans. On the peer, `Origin::Machine` scopes `resolve_task_id` to the sender's `repository` checkout so identifiers are unambiguous.
- **Routed `Read` variants (when `Origin::Local` and `remote_task(issue)` names another Machine):** File operations follow the execution route immediately after a move claim. Only status uses `remote_ownership_writer` so the source can report an unfinished move.
  - `task.status` (replaces `waves::remote_task_detail`), `task.get`, `task.detail`, `task.changes`, `task.diff`, `task.file`, `task.files`, plus `task.runs` and `session.list` when `task: Some(issue)` is set.
  - Unscoped `session.list { task: None }`, `plan.list`, `desktop.view.list`, `view.select`, and `wave.plan` stay local (inventories remain local; zero SSH fanout on list/board reads).
- **Routed `Write` variants:**
  - Add `Write::TaskSave` to `Write::remote()` (`launching: false` — sends no `forward.env` secrets and delivers no `forward.files`).
- **Routed subscriptions (`task.get`, `task.status`, and `task.files.watch`):**
  - **Finding:** Mobile (`TaskWorkspaceView.swift:64`) subscribes to `task.get`, and Desktop (`TaskWorkView.swift:501`) subscribes to `task.get` for remote Graph logs. Currently `Read::subscribe` only reads local SQLite. Meanwhile Desktop (`TaskFileDocument.swift:669`) spawns `lf --machine … task watch-files` via `RegistryQueryLocal.swift:30`.
  - **Decision (made for review, see `scratch/questions.md`):** Forward `task.get` and `task.status` subscriptions to the owning Machine over a dedicated peer `Subscribe` connection when `Origin::Local` and another Machine owns the Task, and add a subscribed `Read::TaskFilesWatch { issue }` backed by `ops/task/workspace.rs`'s existing `notify` watcher so Desktop remote file invalidation also travels over the single server protocol (deleting `RegistryQueryLocal`'s `watchFiles` and `subscribeRead` SSH subprocesses). Watch local `task_ownership` revisions beside the peer stream; when ownership changes, re-route the same subscription to the new owner (or to the local reader when it is now local) and keep its `id`. `task.files.watch` emits one whole-tree invalidation on re-route.
- **Absent and error typing:**
  - **Finding:** `ops/task.rs:386` (`file_context`) returns an untyped `OpsError::Message("Task … is not checked out on this Machine")`, which `server/service.rs:1261` turns into `ErrorCode::Unavailable` — making an unstarted Task indistinguishable from an unreachable Machine!
  - **Done (05968c2e1c):** `file_context` returns typed `engine::Error::new(ErrorCode::NotFound, format!("Task {issue} is not started")).with(json!({ "reason": "not_started", "task_id": task_id }))` when the Task exists on its owner but has no `task_checkouts` row.
- **Protocol revision:** Bump server protocol `REVISION` and `MIN_REVISION` from 9/8 to 10/10 (matching Loopflow's no-backcompat wire rule in `server/README.md`). Regenerate `tests/fixtures/dto/server_schema.json`, update fixtures under `rust/loopflow-execution/tests/fixtures/dto/server/`, and update `swift/Loopflow/Services/ServerProtocol.swift` and `ServerProtocolTests.swift`.

### 3. Terminal ticket bridging and Task shells (`server/service.rs`, `server/ticket.rs`, `server/remote.rs`, `machine/wire.rs`, `desktop/shells.rs`)

- **Finding 1 (Session ownership discovery):** A Session created natively on a Modal sandbox has no row in the laptop's `agent_sessions` table. If `session.attach` / `session.continue` carry only `session: LfSessionId`, the laptop server cannot tell which peer Machine owns it.
  - **Fix:** Add required-nullable `task: Option<String>` and `machine: Option<MachineId>` to `SessionAttach`, `SessionContinue`, and `SessionStart`.
  - Routing resolution when `Origin::Local`:
    1. If `task` is `Some(issue)`: resolve `task_execution_route`. If another Machine owns it, route there.
    2. Else if `machine` is `Some(m)` and `m != local_machine`: route to `m`.
    3. Else check local `conversation_task(&store, &session)` -> `task_execution_route` and `session_moved_to(&store, &session)`. If another Machine owns it, route there.
    4. Else run locally.
- **Finding 2 (Task shells over the server):** Today `desktop/shells.rs` runs via `lf --machine <id> --repo-id <r> desktop shell <task> --checkout <path> --id <tab>` over SSH, and `session.attach` requires a `LfSessionId`, so Mobile has no Shell access and Desktop shells bypass the server.
  - **Fix:** Add three `ServerOperation` variants mirroring `session.continue` + `session.attach`:
    - `task.shell.start { issue, shell_id, rows, cols, bootstrap: Option<String>, command: Vec<String> }` -> `SessionStarted { terminal_id }`. Idempotent by `shell_id` via `desktop_shells`; forwards to owner when remote; returns `not_found { reason: "not_started" }` if no checkout exists on the owner; starts `$SHELL` (with optional Ghostty `bootstrap` JSON for Desktop, `null` for Mobile) in the owner's checkout inside `Registry`.
    - `task.shell.attach { issue, shell_id, terminal_id: Option<TerminalId> }` -> `SessionAttached { ticket, terminal_id, expires_in_ms }`. Never starts a shell; attaches to existing held terminal or returns `not_found` (ended) / `unavailable` (holder waiting).
    - `task.shell.close { issue, shell_id }` -> `{}` (closes shell on owner).
  - A `shell_id` names a receipt on the Machine that started it. After a move, the new owner has no such receipt and answers `not_found` (ended); Desktop and Mobile offer **New shell**. No shell follows a Task to another Machine.
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
- **No client-side location:**
  - Desktop sends the Task id; it never resolves, passes or compares an owner. Delete `SessionsWorkspace.location`, `confirmLocation`, `retainLocation` and the placement guard at `SessionsView.swift:716` with the rest of the location stack: the server attaches the Session wherever it runs.
  - Panes label their Machine from what the server answered: Files/Changes from routed `task.get`'s `reference.workspace.machine_id`, terminals from the attach answer. When a reading names a different Machine than the pane last showed, Files and Changes reload; a Diff or file view with no draft reloads in place.
  - Unsaved drafts stay keyed by `(Task, path, base revision)`. Saving after a move goes to the new owner; a revision mismatch shows the existing save conflict with both versions and keeps the draft.
  - When a Task is unstarted (`not_found` with `reason == "not_started"`), render **Not started** (no empty diff, no local shell). When the owner is unreachable (`unavailable` with `resource == "machine"`), render an explicit error naming the Machine with **Retry**.
  - For Task shells: `SessionsWorkspace.shellAttachment` runs local `lf desktop shell <issue> --id <tab.id> [--create --bootstrap <json>]` (no `--machine`, no `--checkout` local path assumptions), and `closePane` calls `task.shell.close` through `ServerClient` instead of spawning `lf --machine ... desktop shell --close`.
  - For Task Sessions: `Write::SessionResume` returns a `SessionRecord` whose `open_argv` attaches via the local server (`lf session attach --session <id> --task <task>`), eliminating raw `ssh` argv from `prepared_remotely` for Desktop.

### 6. Mobile (`swift/LoopflowiOS/TaskWorkspaceView.swift`, `RemoteMobileClient.swift`, `SessionLink.swift`)

- **Single pairing, four viewers:** Keep the existing single Tailscale pairing to the user's primary Machine server. All four viewers (**Session**, **Files**, **Changes**, **Shell**) talk to that paired server, which routes to the Task's owning Machine transparently.
- **Finding (Mobile primary Session discovery on remote Tasks):** `TaskWorkspaceView.swift:87-106` calls `client.task(id:directory:)` (`desktop.view.get`) on the paired laptop, which reads local SQLite and returns `primarySession == nil` for a remote-Machine Task.
  - **Fix:** Derive the remote Task's primary Session directly from the routed `task.get` subscription frame (`reading.work.sessions`, selecting the primary/open interactive Session, or expose `primary_session` on `DesktopTaskPane` / `TaskOverview`!) AND pass `task: task.id` on `session.attach` and `session.continue` in `RemoteMobileClient` so the paired server routes them to the owning Machine!
- **Files viewer (tree + unchanged files):**
  - Add `TaskFilesTreeView` inside `TaskWorkspaceView` (accessible from the Task workspace segmented control / toolbar alongside Session, Changes, and Shell).
  - Calls `client.query.taskFiles(issue: task.id, path: dir, ...)` to browse directories (with pagination via `next_cursor`) and opens any file (including **unchanged** files) in the existing `TaskFileView` (`client.query.taskFile(issue: task.id, path: path)`).
- **Shell viewer:**
  - Add `TaskShellView` + `ShellLink` (sharing `RemoteAttachment`, `SessionTerminal`, `GhosttySurface`, and the reply/special-keys bar with `SessionView`) inside `TaskWorkspaceView`.
  - Uses a stable per-Task mobile shell ID (e.g. `mobile-<task.id>`), calling `task.shell.attach` first and `task.shell.start` on explicit Start / first open (`bootstrap: nil`, `command: []`, `rows`/`cols` from `TerminalGeometry`), then connecting over `/v1/ticket` WebSocket with the bridged ticket!
- **Absent and error states on Mobile:**
  - Unstarted Task (`not_found { reason: "not_started" }` or `reading.checkout == nil` on an unstarted Task): show **Not started** on Files, Changes, and Shell (never `"No changes yet"` or a local shell).
  - Unreachable owner (`unavailable` with `resource == "machine"`): show explicit error naming the unreachable Machine + **Try again** (never an empty diff or local shell).
  - Task moved: the routed `task.get` subscription's next reading names the new Machine; the workspace header updates and Files/Changes reload. A Shell from the old Machine answers ended and offers **New shell**.

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

- **Server protocol (`machine/wire.rs`, `server/wire.rs`, `server/schema.rs`, `tests/fixtures/dto/server_schema.json`, `rust/loopflow-execution/tests/fixtures/dto/server/*.json`):** Revision 10; `task`/`machine` on `session.attach`/`continue`/`start`; `task.shell.start`/`attach`/`close`; `task.files.watch` subscription; structured `not_started` (`not_found`) and `machine` (`unavailable`) errors.
- **Rust CLI & server (`loopflow`, `loopflow-execution`):** `server/peers.rs`, `server/service.rs`, `server/ticket.rs`, `server/remote.rs`, `server/subscriptions.rs`, `machine/client.rs`, `lf/commands/read.rs`, `lf/commands/write.rs`, `lf/commands/waves.rs`, `lf/commands/open.rs`, `lf/commands/open/conversation.rs`, `lf/commands/desktop.rs`, `lf/navigation.rs`, `bin/lf.rs`, `ops/task.rs`, `ops/task/snapshot.rs`, `desktop/shells.rs`.
- **Shared Swift (`swift/Loopflow`):** `Services/ServerProtocol.swift`, `Services/RegistryQuery.swift`, `Models/WaveWorkMap.swift`, `Models/TaskWork.swift`.
- **Desktop macOS (`swift/LoopflowMac`):** `WorkModel.swift`, `Views/SessionsView.swift`, `Views/WorkspacePanesView.swift`, `Views/TaskWorkView.swift`, `Views/TaskFilesView.swift`, `Views/TaskChangesView.swift`, `TaskFileDocument.swift`, `Services/RegistryQueryLocal.swift`, `Services/ServerClient.swift`.
- **Mobile iOS (`swift/LoopflowiOS`):** `TaskWorkspaceView.swift`, `RemoteMobileClient.swift`, `SessionLink.swift`, `SessionView.swift` (shared terminal chrome).
- **Docs:** `docs/surfaces.md`, `docs/workspaces.md`, `docs/server.md`, `docs/ops.md`, `docs/lf-reference.md`, `rust/loopflow/src/server/README.md`, `docs/dev/machines.md` (updating the client-side SSH note on line 247-252 and server routing description).

## Docs written ahead (write-docs, 2026-11-23)

Describe the intended behavior; implementation must match them or change them.

- `docs/workspaces.md` — new "Open a Task that runs on another Machine" (`lf open --task`, `--task --session`, `--machine --session`, both flag orders, failure table); dropped "Rex can reach remote, Herdr and cmux stay local".
- `docs/surfaces.md` — new "A Task on another Machine" under Loopflow Desktop (four views, Not started / unreachable / moved / draft-on-move table).
- `docs/server.md` — server reaches other Machines' Tasks; phone gets Session, Files, Changes, Shell over one pairing.
- `docs/ops.md` — `--machine` exception for `lf open`; remote Tasks need no `--machine` to inspect.
- `docs/dev/machines.md` — replaced the client-side transport paragraph with server routing.
- `rust/loopflow/src/server/README.md` — "Tasks on another Machine" (routed vs local table, portable Task selector, peer pool, subscription re-route, `not_started` / `machine` errors), revision 10, `task.shell.*` + `task.files.watch` rows, routed terminal tickets. Heading is now `## Local protocol` (no revision number); stale `#local-protocol-revision-7` links fixed there and in `docs/dev/server.md`.
- Slice 2 must make `documented_commands` pass: the `lf open --machine code-1 --session …` example in `docs/workspaces.md` fails Clap until `OpenSelection.machine` exists.

**Open question (from writing the docs):** after a move, a Shell already open keeps running on the old Machine, but reattaching by `shell_id` (Desktop relaunch, phone reopen) routes by Task to the new owner, which answers `not_found`, so a still-running shell becomes unreachable and is never closed. Should `task.shell.attach` reach the Machine that started the shell (record `machine` beside `shell_id` on the client, or keep a local receipt of where each shell started), or should a move close the old Machine's Task shells? The docs currently say "reopened after the move, it shows ended and offers **New shell**".

## Absent and error states

| State | Wire `code` & `data` | CLI (`lf open` / `lf task ...`) | Desktop | Mobile |
| --- | --- | --- | --- | --- |
| Task unstarted (no checkout on owner) | `not_found` `{ "reason": "not_started", "task_id": "…" }` | Explicit message: Task is not started | **Not started** in Files, Changes, Shell (never empty diff / local shell) | **Not started** in Files, Changes, Shell |
| Owner Machine unreachable / SSH failed | `unavailable` `{ "resource": "machine", "machine_id": "…", "machine_label": "…", "machine_name": "…" }` | Retryable failure naming Machine + `ssh <route> lf server status` | Explicit error naming Machine + **Retry** | Explicit error naming Machine + **Try again** |
| Task moved | none: subscriptions re-route; next reading's `reference.workspace.machine_id` names the new owner | Next command routes to the new owner | Header names the new Machine; Files/Changes reload; drafts save if the base revision still matches, else save conflict keeps them; old-Machine terminals keep running, labelled | Header updates; Files/Changes reload; old Shell answers ended → **New shell** |
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
- `swift/LoopflowMac/Views/SessionsView.swift`: `SessionsStore.executionLocations`, `SessionsWorkspace.location`, `confirmLocation`, the placement guard ("The conversation belongs to a different Machine or checkout"), `retainLocation`, `query(for:)`, `onMachine`/`pinTaskLocation` branches in `files(for:task:query:)`, `openWarmConversation`, `loadPaneIntent`, and raw `--machine ... desktop shell` CLI invocation in `shellCommand`.
- `swift/LoopflowMac/WorkModel.swift`: `LinkedSession.location`, `onMachine` / `taskLocation` double-SSH probe in `prepareLinkedTask`.
- `swift/LoopflowMac/Views/TaskWorkView.swift`: `onMachine` / `pinTaskLocation` branch in `GraphProcessLog`.
- `swift/LoopflowMac/TaskFileDocument.swift`: `query.remoteMachine` branches and `observeRemoteFiles` subprocess watcher (replaced by server-routed `task.files.watch` subscription).
- `rust/loopflow/src/lf/commands/ssh.rs`: `prepared_remotely` wrapping `open_argv` in raw `ssh` for Desktop (replaced by local server-routed `lf session attach` while keeping CLI `lf --machine <m> resume` interactive SSH working if invoked directly).
- Obsolete Swift tests asserting `RegistryQuery.onMachine` / `pinTaskLocation` subprocess args (replaced with server-routed tests in `RemoteTaskWorkspaceTests` and updated `TaskFilesTests` / `RegistryQueryTests`).

**Must survive:**
- Local Task Files, Changes, Diff, Save, Shell, Session, and FSEvents watching on the local Machine.
- Unsaved draft preservation across reconnects and Task moves, keyed by `(Task, path, base revision)`; `task.save`'s revision check is the only write guard.
- `lf task location --peers --json` diagnostic CLI command and `tests/fixtures/dto/task_locations.json`.
- `lf --machine <label> <cmd>` SSH transport for all non-`open` commands (and `lf --machine <label> resume` direct CLI transport).
- Mobile's existing single-pairing Tailscale admission, Task creation journal, and Session terminal controls (`Continue here`, reply bar, special keys).

## Slices

Build thinly end to end in four slices; each slice runs and proves itself before moving on, and deletes what it replaces as soon as it works:

1. **Slice 1 (de-risks the hardest core): Server peer pool, routed Task reads/writes/subscriptions, and bridged terminal tickets (`Session` + `TaskShell`).**
   - The first end-to-end piece routes one-shot file browsing, content, changes, diff and revision-checked save over the laptop server. The existing status command uses the same peer connection setup. The route carries the portable selector from the same SQLite row as the Task ID. The receiving server resolves it in the sender's repository before reading or writing.
   - A warm SQLite lookup of a tracker key across 10,000 synthetic Tasks in 100 repositories averaged 59.7 ms over 100 reads on this Machine. With indexed prefix and UUID predicates, the repository-scoped lookup averaged 6.1 µs. `EXPLAIN QUERY PLAN` changed from a Task index scan to point lookups and a primary-key range. These times cover selector SQL only.
   - `cargo test -p loopflow --test integration machine_commands::server_routed_task_workspace` passed against two isolated servers. The test moves a Task with a draft open, checks the owner has no checkout, then browses and edits its files and cuts the SSH route.
   - `cargo test -p loopflow-storage --lib query_plans::tests::task_workspace_selectors_resolve_by_index` passed with both query plans. The existing CLI regression `machine_commands::a_status_read_for_a_task_mini_runs_shows_a_rename_made_there` passed for human and JSON output.
   - `cargo fmt` and `cargo clippy --all-targets -- -D warnings` passed. Gate runs the remaining Task acceptance checks as their slices land.
   - First implementation (05968c2e1c): `Read::peer` (`commands/read.rs`) routes `task.status/changes/diff/file/files` and `Write::TaskSave` to the owner over one-shot `Connection::open_on`; `waves::remote_task_detail` is deleted; `Origin::place` (`server/service.rs`) translates the selector with `work_route::peer_task_id`.
   - The server now pools one-shot reads to the owning Machine, including `task.get`, `task.detail`, `task.runs` and Task-scoped `session.list`. Each request carries its repository. A peer restart retires the idle connection and a read can reconnect once. Bare CLI reads and writes still use one-shot connections.
   - `cargo test -p loopflow --test integration machine_commands::server_routed_task_workspace` passed in 26.17 s. Mini’s `lf server status` showed the same client ID and five additional requests across two repositories sharing the label `WORK-42`. The laptop returned mini’s workspace path, Graph run, and Session, previewed an oversized JSON draft over tickets, reconnected after a peer restart, and named mini on outage. The unscoped Session list stayed local.
   - Open server subscriptions for `task.get`, `task.status` and `task.files.watch` now follow ownership. One planning-revision watcher wakes the routed readers. Each remote reader holds a dedicated peer subscription. File watching returns a whole-tree revision, so coalescing a batch of edits still tells the client to refresh. Served `task.save` shares the read pool and forwards the incoming request ID.
   - `cargo test -p loopflow --test integration machine_commands::server_routed_task_workspace` passed in 47.02 s. The same file and pane subscriptions followed mini → laptop → mini. The status subscription saw edits made on both Machines. Mini counted the two saves on the pooled read client and stored the named save's answer. After a peer restart a named save reconnected, and a disconnected unnamed save returned `outcome_unknown`. All 11 checks in `server::schema::tests::` and `lf::commands::write::tests::` passed.
   - Route `session.attach`, `session.continue`, `session.start` and the three `task.shell` operations. Bridge terminal tickets over the Unix socket and WebSocket.
   - Fix `waves.rs::task_reference` and `ops/task/snapshot.rs` so stale local checkouts never masquerade as active when another Machine owns the Task.
   - Bump protocol to revision 10, regenerate `tests/fixtures/dto/server_schema.json`, update server fixtures and `ServerProtocol.swift` / `ServerProtocolTests.swift`.
   - **Proves:** A two-Machine Rust integration test (`machine_commands::server_routed_task_workspace`) with a stale local checkout (distinct file bytes) reads remote unchanged files, uncommitted diff, saves a remote file, starts + attaches a remote Task shell (`hostname` & `pwd`) and remote Session through the local server's Unix socket AND WebSocket carrier, reuses the pooled peer connection across reads, re-routes an open `task.get` subscription to the new owner when the Task moves (same `id`, next reading names the new Machine), saves a draft after the move only when its base revision matches, returns `not_found/not_started` before checkout, and `unavailable` naming the peer when unreachable.

2. **Slice 2: CLI `lf open` always local + remote Task/Session attachment.**
   - Add `--machine` to `OpenSelection` (distinct Clap id), exempt `open` from remote SSH execution in `navigation.rs` / `bin/lf.rs` for both flag positions (`lf open --machine <label> ...` and `lf --machine <label> open ...`).
   - Fix `open.rs::resume_argv` keeping `--task` + owner Machine/repo when `--session` is set, ignore stale local checkout when `execution_machine.is_some()`, and support remote-only Sessions (`lf open --machine <label> --session <sid> --tab`) with `Attachment` receipt and retryable unavailable errors.
   - **Proves:** Integration test opening a remote Task (`--task <id> --tab` with a stale local checkout), a specific Session on a remote Task (`--task <id> --session <sid> --tab`), and a remote-only Session (`--machine <label> --session <sid> --tab` in both flag positions), all presenting locally and attaching on the peer.

3. **Slice 3: Desktop migration + deletion of `RegistryQuery.onMachine` / `pinTaskLocation`.**
   - Switch Desktop Files, Changes, Diff, Save, Watch, Graph log, Shell, and Session onto local `ServerClient` routed operations addressed by Task id alone.
   - Delete the client location stack, including the placement guard at `SessionsView.swift:716`; reload Files/Changes when a reading names a new Machine; drafts stay keyed by `(Task, path, base revision)`.
   - Delete `RegistryQuery.onMachine`, `pinTaskLocation`, `remoteMachine`, `executionLocations`, and `RegistryQueryLocal`'s remote SSH subprocess runners; update `lf desktop shell` and `prepared_remotely` to attach through the local server ticket bridge.
   - **Proves:** `RemoteTaskWorkspaceTests` on macOS exercising all four viewers (Files unchanged content, Changes uncommitted hunk + Save, Shell `hostname`/`pwd`, Session input acknowledgement) against distinguishable local vs remote server responses, plus disconnect/reopen, unstarted, unreachable, and a Task move (views follow; a draft at a stale base revision shows the save conflict and survives).

4. **Slice 4: Mobile Files & Shell viewers + Docs + Gate.**
   - Expose Files (directory tree + unchanged file content via `task.files` / `task.file`) and Shell (via `task.shell.start` / `task.shell.attach` + `/v1/ticket` WebSocket) alongside Changes and Session in `TaskWorkspaceView.swift` / `RemoteMobileClient.swift`.
   - Derive remote Task primary Session from routed `task.get` and pass `task` on `session.attach` / `session.continue`. Show **Not started** and explicit unreachable-Machine errors.
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
- `server_routed_task_workspace` passes: pooled peer reuse (verified by peer client/connection count), remote unchanged file + uncommitted diff + remote save, bridged Session and Shell tickets over both Unix socket and WebSocket, `not_started` vs `unavailable` (naming Machine), subscription re-route on move, and stale local checkout never exposed in `RoadmapTask`.
- `open_remote_workspace` passes: `lf open --task <id> --tab`, `lf open --task <id> --session <sid> --tab`, and `lf open --machine <label> --session <sid> --tab` (both flag orders) present locally and attach on the peer despite a stale local checkout.
- Desktop and Mobile `RemoteTaskWorkspaceTests` pass: all four viewers (Files unchanged file, Changes uncommitted hunk, Shell `hostname` + `pwd`, Session input ack) succeed through the single server protocol, show **Not started** before checkout and explicit unreachable-Machine errors on outage, and follow a Task move without losing an unsaved draft.

## Demo

With a Task running on a remote Machine (e.g. a Modal rental or SSH peer) and a stale local checkout on the laptop:
1. Run `lf open --task <id> --tab` — opens a local tab attached to the remote Session (not the stale local checkout).
2. Run `lf open --machine <label> --session <remote-sid> --tab` — opens a local tab attached to the remote-only Session.
3. Open the Task in **Loopflow Desktop** — **Files** shows the remote working copy and saves edits to the remote, **Changes** shows the remote uncommitted diff, **Shell** runs `hostname && pwd` on the remote Machine, and **Session** attaches cleanly with live input.
4. Open the same Task on the **paired iPhone** — switch between **Session**, **Files** (browse tree & read an unchanged remote file), **Changes**, and **Shell** (`hostname && pwd`), all over the existing single pairing.
