# LOO-1083 · Server-routed remote Task workspace across CLI, Desktop, and Mobile

**Done when** a Task owned by another Machine (SSH host, pool Machine, or Modal rental) works end to end from CLI, Desktop, and a single-paired phone through the local/paired Loopflow server — with zero local checkout fallback and zero client-side SSH subprocess routing in Desktop:

- Desktop **Files**, **Changes**, **Diff**, **Save**, **Shell**, and **Session** all open against the owning Machine (even when a stale local checkout exists on the Mac). `MTG-421`-style failures (“The conversation belongs to a different Machine or checkout; nothing was opened.”) and local-Mac shell fallbacks are gone.
- Mobile exposes **Files** (directory tree + file content, including unchanged files) and **Shell** alongside **Changes** and **Session**, over the existing single pairing.
- `lf open` always presents on the calling Machine: `lf open --task <id> --tab` and `lf open --task <id> --session <sid> --tab` open a local tab attaching on the owning Machine, and `lf open --machine <label> --session <sid> --tab` (or `lf --machine <label> open ...`) attaches a remote-only Session locally.
- Unstarted Tasks show **Not started**, unreachable owners show an explicit error naming that Machine (never an empty diff or local shell), and a Task that moves is followed: open Task views re-read from the new Machine on their own, terminals already running on the old Machine stay labelled with it, and an unsaved draft saves to wherever the Task is now only if the file there is still at the draft's base revision.

Derived from the Infrastructure Wave KRs: advancing and inspecting Tasks without Loopflow's plumbing blocking or misleading the person.

The Task brief says “Ownership selects the execution Machine” and “`lf open` always presents on the calling Machine” (`.lf/prompts/20261123-211925-f4be887c-cb6f-45ac-bbbe-bdc514fccc5d-pCD10W-summary.md`). The Desktop regression is the MTG-421 error quoted above.

## The model (concept review, 2026-11-23)

**The Task is the address.** A person opens a Task; the Task is on one Machine; every view shows that Machine's checkout. No client holds a second opinion about where the Task is, so nothing needs comparing.

- Clients name the Task (and, for a Session with no Task here, its Machine). Only the local `lf server` resolves ownership and routes. There is no `expected_owner` field, no `conflict` answer, and no client-side "location".
- A subscription follows the Task. When local `task_ownership` advances, the server reopens the stream on the destination under the same subscription ID. The source can report a null workspace before the subscriber learns the move. The destination reading carries `reference.workspace.machine_id`, and `task.files.watch` emits a whole-tree invalidation.
- A draft belongs to `(Task, path, base revision)`. `task.save` already refuses when the file's content revision differs (`ops/task/file_save.rs`), which is the only guard a write needs: identical content on the new owner saves there; different content is a conflict that keeps the draft.
- A terminal is a process on one Machine. Moving the Task neither kills nor retargets a Shell or Session already running; its pane names its Machine and ends on its own `0x07` close.

Passed over: the earlier `expected_owner` compare-and-swap on 14 operations with drafts held at `(machine_id, checkout)`. It made the client a second router, turned a move into an error on every open pane, and kept the guard (`SessionsView.swift:716`) that produced MTG-421's "The conversation belongs to a different Machine or checkout; nothing was opened."

## Source of truth

- **Task ownership:** the append-only `task_ownership` log, resolved via `task_execution_route` (`rust/loopflow-execution/src/store/ownership.rs`, `machine/task.rs::remote_task`). Ownership alone selects the execution Machine.
- **Checkout & execution state:** per-Machine `task_checkouts`, `agent_sessions`, `desktop_shells`, and held terminals (`transport::registry::Registry`) on the owning Machine. A stale `task_checkouts` row on a non-owning Machine is history only, never the active workspace.
- **Cross-Machine router:** the local/paired `lf server` (`rust/loopflow/src/server/`). Every native client (Desktop over Unix socket, Mobile over Tailscale WebSocket, CLI relays/reads) talks to that one server; the server talks to peer Machines over pooled `ssh … lf server connect` connections.

## Server behavior in place

### 1. Pooled peer connections (`rust/loopflow-execution/src/machine/client.rs`, `rust/loopflow/src/server/peers.rs`)

The server shares idle peer connections across repositories, passing a `Sender` on each request. It can retry a read once after a reused pipe disconnects. A write can use the same retry when the peer can deduplicate its request ID. An unnamed write sent before a disconnect returns `outcome_unknown`. Bare CLI requests still use one-shot connections.

### 2. Task-scoped read, write, and subscription routing (`commands/read.rs`, `commands/write.rs`, `server/subscriptions.rs`, `commands/waves.rs`)

- Linear and Asana peers can assign different local Task IDs to one issue. `work_route::peer_task_id` sends a tracker key or identifier, and the peer resolves it in the sender's repository. Trackerless peers use the Task ID.
- Files and saves follow the execution route as soon as this Machine sees the claim. Status can still use the move writer to report an unfinished move. Unscoped inventories stay local.
- `task.get`, `task.status` and `task.files.watch` follow ownership. A broken peer stream publishes its outage and retries after five seconds or a planning change. A refusal such as `not_found` with `reason: not_started` ends the subscription with that error.
- The server, peers and Swift clients negotiate revision 10. Update each Machine that serves a workspace for the installed check.

### 3. Terminal ticket bridging and Task shells (`server/service.rs`, `server/ticket.rs`, `server/remote.rs`, `machine/wire.rs`, `desktop/shells.rs`)

- A Session native to a peer can be absent from the laptop's `agent_sessions`. `session.attach` and `session.continue` accept a Task or an explicit Machine for that case. `session.start` uses the caller's local `argv`, `cwd` and `env`.
- `task.shell.start`, `attach` and `close` run on the owner. Start reuses an existing `shell_id`, and attach opens the held terminal. The Machine that starts the shell keeps its receipt after a move.
- A routed attach wraps the peer's ticket in a single-use ticket bound to the caller's admission. The local server redeems it and forwards terminal input, output, resize and close frames through the same bridge for the Unix socket and WebSocket.

### 4. Ownership-driven `RoadmapTask` / `TaskReferenceSnapshot` (`rust/loopflow/src/lf/commands/waves.rs`)

- The local plan row names the owner in `runtime.owner`. Its `reference.workspace` is null while a peer owns the Task, even if a local checkout still exists. A routed `task.get` returns the owning Machine’s row and checkout.
- The same ownership rule applies to local Task snapshots and the Task record in status. The checkout row stays in storage.

## CLI behavior in place

Both positions of `--machine` select the same registered Machine for `open`. A remote tab starts local `lf open --here` with the Task or Machine and exact Session selectors. The tab command can be replayed, and its host uses a local presentation directory. The original plan sent owner and repository selectors through a remote resume command. The Task selector alone is enough for the server to route a Task Session, and a Machine-only Session needs only its Machine and Session IDs.

The owner's `task.get` supplies the checkout and validates an exact Session's Task membership. For a primary, `session.resume` uses the existing resolver on the owner. The viewer requests `session.attach` first and calls `session.continue` only when the refusal identifies the Session as having no running terminal. The next attach names the terminal just started. The tab reports usable when the peer's terminal acknowledges readiness. The retry command retains the selectors.

## Client work remaining

### 5. Desktop (`swift/LoopflowMac`)

Built and its location stack deleted; see "Slice 3 status". `SessionsWorkspace.location` stays as the last known Machine and checkout (shell label, Files fallback); nothing compares it (`scratch/questions.md`). Shell tabs keep the Machine of the reading they opened in and Close passes it as `--started-on`; `SessionAttached` gained no `machine`.

Remaining, driven by `RemoteTaskWorkspaceTests`:
- Changes and Shell render the server's `not_started` text instead of **Not started**.
- Files' outage shows the routed error without a **Retry**.
- The Files header shows the Machine ID instead of its label.

### 6. Mobile (`swift/LoopflowiOS/TaskWorkspaceView.swift`, `RemoteMobileClient.swift`, `SessionLink.swift`)

- **Single pairing, four viewers:** Keep the existing single Tailscale pairing to the user's primary Machine server. All four viewers (**Session**, **Files**, **Changes**, **Shell**) talk to that paired server, which routes to the Task's owning Machine transparently.
- `TaskWorkspaceView.openConversation` asks the paired laptop's `desktop.view.get` for the primary, which can be absent locally. The routed `task.get` overview lists Session membership and carries no primary ID. Project the owner's saved primary through that reading using the existing Task primary selection. Use it for initial open and later readings, and pass the Task ID to `session.attach` and `session.continue` in `RemoteMobileClient`. Share the selection with the CLI where it fits.
- **Files viewer (tree + unchanged files):**
  - Add `TaskFilesTreeView` inside `TaskWorkspaceView` (accessible from the Task workspace segmented control / toolbar alongside Session, Changes, and Shell).
  - Calls `client.query.taskFiles(issue: task.id, path: dir, ...)` to browse directories (with pagination via `next_cursor`) and opens any file (including **unchanged** files) in the existing `TaskFileView` (`client.query.taskFile(issue: task.id, path: path)`).
- **Shell viewer:**
  - Add `TaskShellView` + `ShellLink` (sharing `RemoteAttachment`, `SessionTerminal`, `GhosttySurface`, and the reply/special-keys bar with `SessionView`) inside `TaskWorkspaceView`.
  - Uses a stable per-Task mobile shell ID (e.g. `mobile-<task.id>`), calling `task.shell.attach` first and `task.shell.start` on explicit Start / first open (`bootstrap: nil`, `command: []`, `rows`/`cols` from `TerminalGeometry`), then connecting over `/v1/ticket` WebSocket with the bridged ticket!
- **Absent and error states on Mobile:**
  - Unstarted Task (`not_found { reason: "not_started" }` or `reading.checkout == nil` on an unstarted Task): show **Not started** on Files, Changes, and Shell (never `"No changes yet"` or a local shell).
  - On peer outages, show the Machine named in the routed error or the subscription's `unavailable` reading and offer **Try again**.
  - Task moved: the routed `task.get` subscription names the new Machine when it receives its checkout. Keep following through an intermediate null workspace. The workspace header updates and Files/Changes reload. Reattaching an old shell answers ended and offers **New shell**.

## Affected surfaces and consumers

- **Server protocol (`machine/wire.rs`, `server/wire.rs`, `server/schema.rs`, `tests/fixtures/dto/server_schema.json`, `rust/loopflow-execution/tests/fixtures/dto/server/*.json`):** Revision 10; `task`/`machine` on `session.attach`/`continue`; `task.shell.start`/`attach`/`close`; `task.files.watch` subscription; structured `not_started` (`not_found`) and `machine` (`unavailable`) errors.
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
- The remote-open examples in `docs/workspaces.md` pass `documented_commands` (four checks).

After a move, an open shell continues on its original Machine. Reattaching by `shell_id` reaches the new owner, which answers `not_found`. Desktop and Mobile offer **New shell**.

## Absent and error states

| State | Wire `code` & `data` | CLI (`lf open` / `lf task ...`) | Desktop | Mobile |
| --- | --- | --- | --- | --- |
| Task unstarted (no checkout on owner) | `not_found` `{ "reason": "not_started", "task_id": "…" }` | Explicit message: Task is not started | **Not started** in Files, Changes, Shell (never empty diff / local shell) | **Not started** in Files, Changes, Shell |
| Owner Machine unreachable / SSH failed | One-shot `unavailable` error carries Machine data. A subscription reading carries the message in `unavailable` and a null body. | Retryable failure naming Machine + `ssh <route> lf server status` | Explicit error naming Machine + **Retry** | Explicit error naming Machine + **Try again** |
| Task moved | none: subscriptions re-route; the destination reading's `reference.workspace.machine_id` names the new owner | Next command routes to the new owner | Header names the new Machine; Files/Changes reload; drafts save if the base revision still matches, else save conflict keeps them; old-Machine terminals keep running, labelled | Header updates; Files/Changes reload; old Shell answers ended → **New shell** |
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

Desktop's client-side SSH routing and `prepared_remotely`'s raw SSH viewer are gone (Slice 3). Still to go: `lf task watch-files --checkout --request`, its `task_files_frames.json` fixture and rejection test, once Mobile Files uses `task.files.watch`.

**Must survive:**
- Local Task Files, Changes, Diff, Save, Shell, Session, and FSEvents watching on the local Machine.
- Unsaved draft preservation across reconnects and Task moves, keyed by `(Task, path, base revision)`; `task.save`'s revision check is the only write guard.
- `lf task location --peers --json` diagnostic CLI command and `tests/fixtures/dto/task_locations.json`.
- `lf --machine <label> <cmd>` SSH transport for all non-`open` commands (and `lf --machine <label> resume` direct CLI transport).
- Mobile's existing single-pairing Tailscale admission, Task creation journal, and Session terminal controls (`Continue here`, reply bar, special keys).

## Slices

Build thinly end to end in four slices; each slice runs and proves itself before moving on, and deletes what it replaces as soon as it works:

1. **Slice 1 (de-risks the hardest core): Server peer pool, routed Task reads/writes/subscriptions, and bridged terminal tickets (`Session` + `TaskShell`).**
   **Exists** (each run is `cargo test -p loopflow --test integration machine_commands::server_routed_task_workspace` against two isolated servers):
   - Routed one-shot reads (`task.status/get/detail/changes/diff/file/files/runs`, Task-scoped `session.list`) and `task.save` go to the owner through `server/peers.rs::PeerPool`; the route carries the portable selector, resolved in the sender's repository (`work_route::peer_task_id`, `Origin::place`). Bare CLI commands still use one-shot `Connection::open_on`. Unscoped lists stay local.
   - Selector lookup: a warm tracker-key lookup across 10,000 synthetic Tasks in 100 repositories averaged 59.7 ms before indexing, 6.1 µs after (`query_plans::tests::task_workspace_selectors_resolve_by_index`).
   - Subscriptions for `task.get`, `task.status`, `task.files.watch` follow ownership: one planning-revision watcher, `Producer::follow` re-resolves `peer_route` and re-subscribes under the same id. `task.files.watch` emits an opaque whole-tree revision. Run (47.02 s): file and pane subscriptions followed mini → laptop → mini; a named save reconnected after a peer restart; an unnamed save on a broken pipe returned `outcome_unknown`.
   - `session.attach`/`session.continue` route by Task owner, then `machine`, then local Session records (`work_route::terminal_machine`). A routed attach asks the peer via `PeerPool::terminal`, issues `Purpose::RemoteTerminal`; redemption runs `ssh … lf server connect` and hands `TerminalStream::There` to both the Unix-socket and WebSocket bridges. Run (31.76 s): a Session held only on mini is `not_found` without `task` and with `task` bridges through the laptop socket showing `on mini: <mini checkout>`.
   - `task.shell.start/attach/close` route by Task owner over the same peer request and bridge; `desktop::shells::served` starts through `Registry::start` and shares the receipt step with `lf desktop shell`. Run: start on mini through the laptop, a repeated start reuses the terminal, bridge shows `shell on mini: <mini checkout>`, close, then attach answers `not_found`.
   - The re-route that once ended the files watch with “mini is unavailable: deadline has elapsed” now publishes a reading with a null body and the failure in `unavailable`. The file watch recovered under the same ID after a refused handshake during a move. An established watch also answered a checkpoint during a peer outage and delivered a new file revision after restart.
   - The same fixture imported a saved provider thread on mini, continued it by Task from the laptop, and repeated the request with the paired-phone fixture. Both connections received the same terminal ID. Its Session and Task shell showed mini’s checkout over the Unix socket and TLS WebSocket. The provider stand-in answered “received hello on mini” to input from the WebSocket. The focused test passed in 134.53 s. `cargo fmt` and `cargo clippy --all-targets -- -D warnings` passed.

   **Slice 1 complete.** A watch on mini’s unstarted Task ended with `not_found` and reason `not_started`. During a move, the laptop’s plan row kept mini as owner and had a null workspace despite a retained local checkout. Its Task wait snapshot had no checkout, branch or base commit. The pane and file watch both reported mini’s outage and recovered under their original IDs after the claim was exchanged and the peer returned. `machine_commands::server_routed_task_workspace` passed in 45.60 s.

   The first run exposed an omitted return claim in the fixture. Mini still recorded the laptop as owner, so its pane correctly showed no workspace. The fixture now observes that intermediate reading on the outward move and exchanges the return claim before reconnecting.

   The wire, generated schema, server fixtures, and Swift mirror all speak revision 10 only. Ten focused Rust schema, handshake and checkout tests passed. The Swift protocol and compatible-build client checks passed (24 tests). `cargo fmt --all` and `cargo clippy --all-targets -- -D warnings` passed.

2. **Slice 2. CLI opens a remote conversation locally.**

   `open_remote_workspace` exercises a local tab host with two Machine servers. The Task primary, an exact secondary Session and a Machine-only Session in both flag positions each answer typed input from the expected provider thread. The receipt carries the selectors for retry.

   `cargo test -p loopflow --test integration open_remote_workspace` passed in 21.92 s. The fixture prestarted the primary on mini, retained a separate stale checkout on the laptop, and opened the Machine-only Session from outside Git. All four receipts reported usable and kept their retry selectors. The existing local cmux Task-tab check passed in 4.05 s. `cargo fmt --all` and `cargo clippy --all-targets -- -D warnings` passed.

   Upstream #1973 (`00d14c493b`) merged after that check. It records Session membership on the Session and exposes primary scope through `primary_scope`. On the merged branch, `open_remote_workspace` was updated to start the primary provider and verify its terminal before marking `task_primary_sessions` so automatic recovery does not race the prestart; after that fix, `open_remote_workspace` passed in 29.46 s and `documented_commands` (4 tests) passed in 35.13 s. Slice 2's exact-Session check uses the routed overview membership. Desktop fixtures should set Session membership and save the chosen primary, including when the owner's checkout differs from the Session's recorded cwd.

3. **Slice 3. Desktop opens a remote Task through the local server.** Built except the two-Machine Desktop fixture; see "Slice 3 status" below.
   - Switch Desktop Files, Changes, Diff, Save, Watch, Graph log, Shell, and Session onto local `ServerClient` routed operations addressed by Task id alone.
   - Delete the client location stack, including the placement guard at `SessionsView.swift:716`; reload Files/Changes when a reading names a new Machine; drafts stay keyed by `(Task, path, base revision)`.
   - Delete `RegistryQuery.onMachine`, `pinTaskLocation`, `remoteMachine`, `executionLocations`, and `RegistryQueryLocal`'s remote SSH subprocess runners; update `lf desktop shell` and `prepared_remotely` to attach through the local server ticket bridge.
   - **Proves:** `RemoteTaskWorkspaceTests` on macOS exercises all four viewers (unchanged Files, Changes with an uncommitted hunk and Save, Shell `hostname`/`pwd`, Session input acknowledgement), disconnect and reopen, unstarted and unreachable states, and a move through the intermediate null workspace. The old terminal keeps its Machine label, and Close ends the old shell there. A draft with the same base revision saves on the new owner. A draft with a different base revision shows a save conflict and survives.

   **Slice 3 status (2026-11-23).**
   - Built: Desktop Files, Changes, Diff, Save, Graph log, Shell and Session name the Task only; `RegistryQueryLocal` serves every read, write and subscription through `ServerClient`. A checkout on this Mac keeps FSEvents; any other uses `task.files.watch`, whose reading moves the same `TaskFilesStore` (`relocate`) so drafts stay keyed by Task, path and base revision. Deleted: `onMachine`, `pinTaskLocation`, `taskLocation`, `remoteMachine`, the SSH line readers (`LocalLineObservation`, `ReaderLaunchConfiguration`), the Swift `TaskLocationObservation`/`TaskFilesFrame` mirrors, pane `execution`, `SessionsStore.executionLocations`, `LinkedSession.location`, `requiresLocationValidation`/`confirmLocation`, and the placement guard.
   - `lf desktop shell <task> --id <tab>` asks the local server (`task.shell.start/attach/close`); `shells::run` and `--checkout` are gone. `task.shell.close` takes `machine`, and Desktop Close passes `--started-on` the tab's Machine. `prepared_remotely` now answers `lf open --task|--machine … --session … --here`. `relay_socket` honors `LF_VIEWER_STREAM`, so `lf --machine m desktop shell` still works over SSH.
   - Ran: `machine_commands::server_routed_task_workspace` (70.75 s, now with a pre-move shell closed on mini after the move), the three `transport_tests desktop_shell_*`/cleanup tests, `ssh_shells:: -- --ignored` (all three, real loopback sshd; the prepared viewer runs through a source-side server), `remote_session_open_*` and `bare_connect_*`, `lf::tests::`, `server::schema::tests::`, `documented_commands::`, clippy. Swift: 310 tests in the 13 touched suites and 136 in 17 neighbouring suites passed.
   - Taught: the warm opener now prepares any Task through the server, so fixtures that answered every `session resume` with one record raced; they now answer only their Task. Tests that asserted refusals (remote shell without an observation, location-changed restart) now assert the routed behavior.
   - **Next in Slice 3:** `RemoteTaskWorkspaceTests` (two isolated Machine servers behind the Swift app: unchanged file, hunk + Save, Shell `hostname`/`pwd`, Session input, disconnect/reopen, Not started, unreachable, move with draft and old-shell Close). Gaps it should drive: Changes and Shell render the server's `not_started` text rather than **Not started**; Files' outage shows the error without a dedicated **Retry**; the Files header shows the Machine ID, not its label.

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
