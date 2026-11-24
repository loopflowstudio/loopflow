# LOO-1083 · Server-routed remote Task workspace across CLI, Desktop, and Mobile

**Done when** a Task owned by another Machine (SSH host, pool Machine, or Modal rental) works end to end from CLI, Desktop, and a single-paired phone through the local/paired Loopflow server — with zero local checkout fallback and zero client-side SSH subprocess routing in Desktop:

- Desktop **Files**, **Changes**, **Diff**, **Save**, **Shell**, and **Session** all open against the owning Machine (even when a stale local checkout exists on the Mac). `MTG-421`-style failures (“The conversation belongs to a different Machine or checkout; nothing was opened.”) and local-Mac shell fallbacks are gone.
- Mobile exposes **Files** (directory tree + file content, including unchanged files) and **Shell** alongside **Changes** and **Session**, over the existing single pairing.
- `lf open` always presents on the calling Machine: `lf open --task <id> --tab` and `lf open --task <id> --session <sid> --tab` open a local tab attaching on the owning Machine, and `lf open --machine <label> --session <sid> --tab` (or `lf --machine <label> open ...`) attaches a remote-only Session locally.
- Unstarted Tasks show **Not started**, unreachable owners show an explicit error naming that Machine (never an empty diff or local shell), and a Task that moves is followed: open Task views re-read from the new Machine on their own, terminals already running on the old Machine stay labelled with it, and an unsaved draft saves to wherever the Task is now only if the file there is still at the draft's base revision.

Derived from the Infrastructure Wave KRs: advancing and inspecting Tasks without Loopflow's plumbing blocking or misleading the person.

The Task brief says “Ownership selects the execution Machine” and “`lf open` always presents on the calling Machine” (`.lf/prompts/20261123-211925-f4be887c-cb6f-45ac-bbbe-bdc514fccc5d-pCD10W-summary.md`). The Desktop regression is the MTG-421 error quoted above.

## The model (concept reviews, 2026-11-23 & 2026-11-24)

**The Task is the address.** A person opens a Task; the Task is on one Machine; every view shows that Machine's checkout. No client holds a second opinion about where the Task is, so nothing needs comparing.

- Clients name the Task (and, for a Session with no Task here, its Machine). Only the local `lf server` resolves ownership and routes. There is no `expected_owner` field, no `conflict` answer, and no client-side "location".
- Every Task subscription (`task.get`, `task.status`, `task.files.watch`) follows the Task across its whole lifecycle — unstarted, live, and moving between Machines — under the same subscription ID. When local `task_ownership` advances, the server reopens the stream on the destination. While a Task exists but has no checkout on its current owner (unstarted or mid-move before arrival creates the checkout), `task.files.watch` stays open emitting `{ machine_id, machine_label, checkout: null, revision: null }` and wakes from `store_revisions` when `task_checkouts` appears, so clients need no polling loop. One-shot `task.files`/`task.file`/`task.changes`/`task.diff` still return `not_found { reason: "not_started" }`.
- A draft belongs to `(Task, path, base revision)`. `task.save` already refuses when the file's content revision differs (`ops/task/file_save.rs`), which is the only guard a write needs: identical content on the new owner saves there; different content is held whole as an external conflict that keeps the draft.
- A terminal is a process on one Machine. Moving the Task neither kills nor retargets a Shell or Session already running; its pane names its Machine and ends on its own `0x07` close.
- Caller intent and calling-Machine commands survive the route: `session.resume` and `session.continue` carry the caller's optional `agent` override so `lf open --task <id> -a <harness>` and Desktop's remote agent switcher work even when the owner has no `default_agent` or is already running another harness; `session.new` carries optional `task` and routes through the same Task-first `remote_session` boundary as `session.resume` (resolving explicit `task` ahead of `moved_session` so Task-addressed Session writes never downgrade to `--machine`); leaving or detaching from a bridged remote Session prints the calling Machine's `lf open` command instead of the peer's local-only `lf resume <id>`; and any remote `SessionRecord` returned by the local server (`session.resume`, `session.new`, and routed `session.list --task`) rewrites `open_argv`, `action.help`, `outlook.command`, `status_line`, `task_ids` (this Machine's `task_id` first), `work.id` (when `kind == "task"`), and `workspace.task_id` for the calling Machine.
- Desktop's Session and Browser panes follow `task.runtime.owner` rather than a stale client-held location: an owner change retires the old Machine's Session viewer, clears stale `preparedTaskWorktrees` and Session/Browser pane locations (keeping running `ShellTab`s on their starting Machine), and re-opens the moved Session on the new owner when visible; every Session preparation/adoption syncs `pane.content.location` even when the moved Session keeps its `LfSessionId`; a Task Browser pane never shows a separate-checkout context label in its strip (`showsContext` is false for Task `.browser`); and **New conversation** on a remote Task runs on the owning Machine through `newShell` instead of creating a local checkout.

Passed over: the earlier `expected_owner` compare-and-swap on 14 operations with drafts held at `(machine_id, checkout)` (made clients a second router and kept the MTG-421 guard); keeping `task.files.watch` checkout-scoped and pushing 5-second retry loops into clients (broke mid-move and server-restart recovery); leaving `session.continue` without `agent` while rev 10 is still unshipped.

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
- `task.get`, `task.status` and `task.files.watch` follow ownership. A broken peer stream publishes its outage and retries after five seconds or a planning change. A missing/unknown Task ends with `not_found`. In Slice 5A, `task.files.watch` stays open on an existing Task with no checkout yet (`checkout: null, revision: null`), waking on `store_revisions` when the checkout appears, and carries `machine_label`.
- The server, peers and Swift clients negotiate revision 10. Update each Machine that serves a workspace for the installed check.

### 3. Terminal ticket bridging and Task shells (`server/service.rs`, `server/ticket.rs`, `server/remote.rs`, `machine/wire.rs`, `desktop/shells.rs`)

- A Session native to a peer can be absent from the laptop's `agent_sessions`. `session.attach` and `session.continue` accept a Task or an explicit Machine for that case; in Slice 5A, `session.continue` also carries required-nullable `agent` so `-a <harness>` and Desktop's remote agent switcher work across Machines. `session.start` uses the caller's local `argv`, `cwd` and `env`.
- `task.shell.start`, `attach` and `close` run on the owner. Start reuses an existing `shell_id`, and attach opens the held terminal. The Machine that starts the shell keeps its receipt after a move.
- A routed attach wraps the peer's ticket in a single-use ticket bound to the caller's admission. The local server redeems it and forwards terminal input, output, resize and close frames through the same bridge for the Unix socket and WebSocket.

### 4. Ownership-driven `RoadmapTask` / `TaskReferenceSnapshot` (`rust/loopflow/src/lf/commands/waves.rs`)

- The local plan row names the owner in `runtime.owner`. Its `reference.workspace` is null while a peer owns the Task, even if a local checkout still exists. A routed `task.get` returns the owning Machine’s row and checkout.
- The same ownership rule applies to local Task snapshots and the Task record in status. The checkout row stays in storage.

## CLI behavior in place

Both positions of `--machine` select the same registered Machine for `open`. A remote tab starts local `lf open --here` with the Task or Machine and exact Session selectors. The tab command can be replayed, and its host uses a local presentation directory. The original plan sent owner and repository selectors through a remote resume command. The Task selector alone is enough for the server to route a Task Session, and a Machine-only Session needs only its Machine and Session IDs.

The owner's `task.get` supplies the checkout and validates an exact Session's Task membership. For a primary, `session.resume` uses the existing resolver on the owner. The viewer requests `session.attach` first and calls `session.continue` only when the refusal identifies the Session as having no running terminal. The next attach names the terminal just started. The tab reports usable when the peer's terminal acknowledges readiness. The retry command retains the selectors.

## Client behavior in place

### 5. Desktop (`swift/LoopflowMac`): done

Built and its location stack deleted; see "Slice 3 status". `SessionsWorkspace.location` stays as the last known Machine and checkout (shell label, Files fallback); nothing compares it (`scratch/questions.md`). Shell tabs keep the Machine of the reading they opened in and Close passes it as `--started-on`; `SessionAttached` gained no `machine`.

`RemoteTaskWorkspaceTests` drove the last gaps closed: Files and Changes show **Not started** with **Retry** (`TaskFilesStore.notStarted`), the outage line has **Retry** (`retry()`), the Files header names the Machine by the plan's label, and an ended shell tab asks the server (`task.files`) whether the Task is started and says **Not started**.

### 6. Mobile (`swift/LoopflowiOS`): done

See "Slice 4 status". What follows is the design it was built from.


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
- **Mobile iOS (`swift/LoopflowiOS`):** `TaskWorkspaceView.swift`, `RemoteMobileClient.swift`, `TaskFilesTree.swift`, `TerminalLink.swift`, `TerminalView.swift` (Session and Shell terminal chrome).
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

Desktop's client-side SSH routing and `prepared_remotely`'s raw SSH viewer are gone (Slice 3); `lf task watch-files`, `TaskFilesFrame` and its fixture and observer test are gone (Slice 4).

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

3. **Slice 3. Desktop opens a remote Task through the local server.** Complete; see "Slice 3 status" below.
   - Switch Desktop Files, Changes, Diff, Save, Watch, Graph log, Shell, and Session onto local `ServerClient` routed operations addressed by Task id alone.
   - Delete the client location stack, including the placement guard at `SessionsView.swift:716`; reload Files/Changes when a reading names a new Machine; drafts stay keyed by `(Task, path, base revision)`.
   - Delete `RegistryQuery.onMachine`, `pinTaskLocation`, `remoteMachine`, `executionLocations`, and `RegistryQueryLocal`'s remote SSH subprocess runners; update `lf desktop shell` and `prepared_remotely` to attach through the local server ticket bridge.
   - **Proves:** `RemoteTaskWorkspaceTests` on macOS exercises all four viewers (unchanged Files, Changes with an uncommitted hunk and Save, Shell `hostname`/`pwd`, Session input acknowledgement), disconnect and reopen, unstarted and unreachable states, and a move through the intermediate null workspace. The old terminal keeps its Machine label, and Close ends the old shell there. A draft with the same base revision saves on the new owner. A draft with a different base revision shows a save conflict and survives.

   **Slice 3 status (2026-11-23).**
   - Built: Desktop Files, Changes, Diff, Save, Graph log, Shell and Session name the Task only; `RegistryQueryLocal` serves every read, write and subscription through `ServerClient`. A checkout on this Mac keeps FSEvents; any other uses `task.files.watch`, whose reading moves the same `TaskFilesStore` (`relocate`) so drafts stay keyed by Task, path and base revision. Deleted: `onMachine`, `pinTaskLocation`, `taskLocation`, `remoteMachine`, the SSH line readers (`LocalLineObservation`, `ReaderLaunchConfiguration`), the Swift `TaskLocationObservation`/`TaskFilesFrame` mirrors, pane `execution`, `SessionsStore.executionLocations`, `LinkedSession.location`, `requiresLocationValidation`/`confirmLocation`, and the placement guard.
   - `lf desktop shell <task> --id <tab>` asks the local server (`task.shell.start/attach/close`); `shells::run` and `--checkout` are gone. `task.shell.close` takes `machine`, and Desktop Close passes `--started-on` the tab's Machine. `prepared_remotely` now answers `lf open --task|--machine … --session … --here`. `relay_socket` honors `LF_VIEWER_STREAM`, so `lf --machine m desktop shell` still works over SSH.
   - Ran: `machine_commands::server_routed_task_workspace` (70.75 s, now with a pre-move shell closed on mini after the move), the three `transport_tests desktop_shell_*`/cleanup tests, `ssh_shells:: -- --ignored` (all three, real loopback sshd; the prepared viewer runs through a source-side server), `remote_session_open_*` and `bare_connect_*`, `lf::tests::`, `server::schema::tests::`, `documented_commands::`, clippy. Swift: 310 tests in the 13 touched suites and 136 in 17 neighbouring suites passed.
   - Taught: the warm opener now prepares any Task through the server, so fixtures that answered every `session resume` with one record raced; they now answer only their Task. Tests that asserted refusals (remote shell without an observation, location-changed restart) now assert the routed behavior.
   - `RemoteTaskWorkspaceTests` (2026-11-23) builds both Machines with the CLI only: `machine add mini` over a stand-in `ssh`, `task move --machine mini`, then `lf --machine mini task checkout`, which clones the shared origin there. Through the laptop's server: Not started (Changes, tree, watch, ended shell tab), an unchanged file, mini's uncommitted hunk in Changes, Save onto mini, `lf desktop shell` printing mini's checkout from `pwd`, the Session argv from `prepareSession` answering typed input from mini's provider, an outage naming mini in the watch and a new read, Retry and reopen. The move test keeps a laptop shell running, saves a same-revision draft on mini, keeps a different-revision draft whole, and Close with `--started-on` ends the old shell on the laptop. Both tests passed headless (`scripts/test_desktop.sh --filter RemoteTaskWorkspaceTests`, 76 s; again at `6af0dff787` with its always-run teardown, 92.7 s); the neighbouring Files, panes and workspace suites passed (273 tests).
   - Taught: after a move, the re-read three-way-merged a dirty draft into the new Machine's file, and an edit overlapping the file there was dropped. `TaskFileDocument.arrive` now keeps such a draft whole beside the new file (`external`, saving unavailable); with the fix reverted the move test fails with the draft replaced by "Updated from disk". The headless fixture runs terminals under `script(1)`: Ghostty needs a display, so the pane's argv is what is exercised, not its rendering.

4. **Slice 4: Mobile Files & Shell viewers + Docs.** Complete.
   - Expose Files (directory tree + unchanged file content via `task.files` / `task.file`) and Shell (via `task.shell.start` / `task.shell.attach` + `/v1/ticket` WebSocket) alongside Changes and Session in `TaskWorkspaceView.swift` / `RemoteMobileClient.swift`.
   - Derive remote Task primary Session from routed `task.get` and pass `task` on `session.attach` / `session.continue`. Show **Not started** and explicit unreachable-Machine errors.
   - Update user and dev docs (`docs/surfaces.md`, `docs/workspaces.md`, `docs/server.md`, `docs/ops.md`, `docs/lf-reference.md`, `rust/loopflow/src/server/README.md`, `docs/dev/machines.md`).
   - **Proves:** `LoopflowiOSTests/RemoteTaskWorkspaceTests` exercising all four viewers, unstarted, unreachable, and Task-move handling over one paired server connection.

   **Slice 4 status (2026-11-24).**
   - Built: `TaskWorkspaceView` switches **Session · Files · Changes · Shell** and names the Machine under the workflow sentence when it is not the paired one. `TaskFilesTree` browses `task.files` page by page and follows `task.files.watch`. `TerminalLink` (was `SessionLink`) holds a Session or the shell `mobile-<task>`: `task.shell.attach`, then `task.shell.start` on first open, then the `/v1/ticket` WebSocket. `session.attach`/`continue` carry the Task. The primary comes from routed `task.detail`'s `current_session`. Swift `ServerOperation.taskShellStart/Attach` with shared fixtures `task_shell_start.json`/`task_shell_attach.json`; schema fixture regenerated.
   - Deleted: `lf task watch-files`, `watch_task_files`, `TaskFilesFrame`, `task_files_frames.json`, `remote_file_observer_keeps_the_recorded_owner_and_streams_changes`, the Changes sheet and the "No changes yet" stand-in for an unstarted Task.
   - Ran: `uv run python scripts/test_ios.py -only-testing:LoopflowiOSTests/RemoteTaskWorkspaceTests` passed both tests (31 s, 40 s), and `TaskActionsTests` (4). `ops::task::workspace::tests` (1), `server::schema::tests:: server::wire:: lf::tests::` (48), `documented_commands::` (4), and `scripts/test_desktop.sh --filter 'ServerProtocolTests|DTOFixtureTests|DesignDisciplineTests'` (80) passed.
   - Taught: the Simulator cannot run `lf` (no OS account lookup), so `scripts/ios_fixture_host.py` runs the fixture's commands. A move whose destination has no checkout yet ended the files watch with `not_started` (fixed in Slice 5A below). Held terminals' holders exit 60 s after their last terminal; the fixture closes the phone's shells on both Machines.

5. **Slice 5 (post-build concept review, 2026-11-24): Close the remaining Task-subscription, remote-`--agent`, and Desktop local-assumption gaps before gate.** Review record: `scratch/concept-review.html`.

   **Slice 5 status (2026-11-24).** Complete. Built against the three review findings: Task-scoped files watch, `--agent` through remote resume/continue with this Machine's `lf open` farewell, and Desktop's location-gated shell and "local" Diff chip.
   - 5A built: `TaskFilesWatch` follows the Task record, not a directory: an unstarted Task reads `{checkout: null, revision: null}` and the watch opens on the checkout when `task_checkouts` advances the planning revision (the producer waits on the shared planning watcher beside file events). A peer reading's `machine_label` is rewritten to the serving Machine's name for the owner. `session.continue` carries required-nullable `agent` (`--agent` on the held `lf session resume`). `session::prepared` now selects the Clap spelling's settings (`select_invocation`), which is what QA #1 was: a served `session.resume` dropped `--agent` before `session::conversation` resolved the harness. `routed_terminal` continues directly when `--agent` names another harness than the primary's (or for an exact Session, whose provider the overview lacks). `transport::ReturnCommand` rewrites the owner's `lf resume <id>` in a bridged viewer's detach and hint lines to this Machine's `lf open …`; provider output passes untouched, and readiness answers with the terminal's own detach text.
   - 5B built: Swift `TaskFilesRevision` (`machineLabel`, nullable `checkout`/`revision`, `notStarted`), `sessionContinue(…, agent:)`. Mobile `TaskFilesTree.follow` is one stream; its Not started has no **Try again**. Desktop `TaskFilesStore` takes `localMachineID`, hands a reading for this Mac to FSEvents through `relocate`, sets `notStarted` and the label from readings, and resubscribes with 1 s → 30 s backoff when its server stream ends or fails. `SessionsWorkspace.owner` (set from `runtime.owner` beside `location`) lets `newShell` open before any conversation. `TaskDelivery.describe(runsOn:)` says "Changes below are on mini." / "The checkout on mini is ahead of the PR".
   - Ran: `machine_commands::server_routed_task_workspace` (34 s; unstarted watch goes live under its ID with `machine_label: "mini"`; the move loops wait through null checkouts), `open_remote_workspace` (19 s; a fifth case opens a Task with no conversation via `-a claude`; every tab's text names `lf open …` and never `lf resume`), `server::schema::tests:: server::wire:: server::service::tests::continuing_a_session` (11), `server::subscriptions::` under nextest (14; a `cargo test` collision seen here did not reproduce in Slice 6), fmt, clippy. Desktop `ServerProtocolTests|DTOFixtureTests|PullRequestTests|TaskFilesTests|RemoteTaskWorkspaceTests` (138, then 59 after the move-back change); iOS `RemoteTaskWorkspaceTests` (2).
   - Not covered: switching harness on a *running* remote Session (`lf -a codex open --task` while claude runs) has no fixture: it needs a second harness stand-in on mini. Move-back to this Mac is covered in `TaskFilesTests` (fixture stream), not end to end: the Desktop two-Machine fixture's mini has no route back to the laptop (`lf --machine mini task move … --machine <laptop>` answers "machine … is not added").

6. **Slice 6 (built 2026-11-24): Desktop panes follow the Task's owner; remote `SessionRecord`s open with this Machine's `lf open`.** Review record: `scratch/concept-review.html`. No wire change.
   - **6A:** `ssh::localize_remote_session` (JSON-level, so selected readings keep only their fields) rewrites `open_argv`, `action.help`, `outlook.command`, `status_line` and adds the route's local `TaskId` to `task_ids`. Callers: `prepared_remotely` (`Write::SessionResume`, which now carries its `TaskExecutionRoute` on `Remote`) and `Read::localize_peer_reading`, applied in `read_with` and on each peer subscription frame. A routed resume with a route opens by `--task <peer_selector>`; without one, by `--machine`.
   - **6B:** `SessionsWorkspace.follow(owner:)` clears locations naming the Machine the Task left; `WorkModel.follow` also clears `preparedTaskWorktrees` and releases the old viewer through `SessionsStore.release`. `taskWorkspaceIdentity` (now shared with `SessionsView`) ignores a prepared checkout on another Machine than the owner. `SessionsStore.onPrepared` → `SessionsWorkspaceRegistry.locate` places every pane on each prepare/adopt/switch/reset. Warm opening admits a shown, started Task with an owner and no location; prefetch keeps its local-location guard. Task Browser strips print no checkout while `workspace.location` is nil. Remote **New conversation** opens a shell on the owner running `lf --interactive --task …`.
   - Ran: `lf::commands::ssh::tests::` (4), `machine_commands::server_routed_task_workspace` (45 s, routed `session.list --task` carries the laptop's Task ID, opens with `lf open --task`, no `lf resume`), `server::subscriptions::` under `cargo test` five times (13/13 each; the collision recorded in Slice 5 did not reproduce at this head), clippy. Desktop `RemoteTaskWorkspaceTests` (2, 54 s: the move test follows a same-ID Session pane laptop → mini and drops the Browser's laptop place), plus `WorkspacePanesTests|SessionsStoreTests|WorkModelTests|WorkDestinationTests|TaskSessionLaunchProofTests|WarmPaneMountTests|EmptyStateTests|WorkspaceRetentionTests|DesktopPaneControlTests` (215).
   - Not covered by a test: remote **New conversation** (`SessionsContentView.newTaskSession` is private to the view) and the strip predicate; QA on the Dev build checks both.

7. **Slice 7 (post-build concept review pass 3, 2026-11-24): Complete remote `SessionRecord` Task-ID localization, unify Task-routed `session.resume` + `session.new`, and drop Task Browser strip context.** Review record: `scratch/concept-review.html`. Stays in rev 10 (unshipped).
   - **7A (Rust server & wire):**
     - `ssh::localize_remote_session` (`rust/loopflow/src/lf/commands/ssh.rs`): put `route.task_id` at index 0 of `task_ids` (moving or inserting it first so callers reading `task_ids.first` on tracker-backed peers never pick the peer's local UUID); rewrite `work.id` when `work.kind == "task"` and `workspace.task_id` when non-null to `route.task_id` (mutating only keys present so selected JSON readings stay minimal).
     - `Write::SessionNew` (`rust/loopflow/src/lf/commands/write.rs`): add required-nullable `task: Option<String>` (`#[serde(deserialize_with = "Option::deserialize")]`), regenerate `tests/fixtures/dto/server_schema.json`, and include `Write::SessionNew` in `Write::remote()` → `remote_session()`.
     - `Write::remote_session` & `Write::run_here` (`write.rs`, `work_route.rs`): when an explicit `task` selector is present on `SessionResume` or `SessionNew`, resolve its `TaskExecutionRoute` *before* `moved_session` so Task-addressed writes on moved Sessions keep `Remote.route` (keeping `open_argv` / `action.help` on `--task <peer_selector>` instead of downgrading to `--machine`) and rewrite `task` to `route.peer_selector`; in `Write::run_here`, translate `peer_task_id` for `SessionResume` and `SessionNew` whenever `task` is `Some`.
   - **7B (Desktop & shared Swift):**
     - Add `task: String?` to `ServedWrite.sessionNew` (`swift/Loopflow/Services/RegistryQuery.swift`), update `RegistryQuery.resetSession(..., work: WorkReference?)` and `RegistryQueryLocal.swift`, and pass `work: preparationWork(previous.record)` from `SessionsStore.reset` (`swift/LoopflowMac/Views/SessionsView.swift`). Update `RegistryQueryTests.swift` and `write.rs` encoding tests.
     - Simplify `WorkspacePanesView.showsContext` (`swift/LoopflowMac/Views/WorkspacePanesView.swift`): return `false` for any Task `.browser` pane (Files already shows `mini origin.…` in `files-location`; eliminates the duplicate `"Files"` label in `scratch/qa/s6-final-move.png`).
   - **Proves:** Rust unit tests for `localize_remote_session` (local `task_id` at index 0, `work.id`, `workspace.task_id`, non-task `work`, selected readings) and `Write::SessionNew` schema/encoding; `machine_commands::server_routed_task_workspace` verifying routed `session.new` on a remote Task (including `open_argv` / `task_ids` / `work.id`) and a moved Session's `session.resume { id, task }` keeping `--task` in `open_argv` and `action.help`; Swift `RegistryQueryTests` + `RemoteTaskWorkspaceTests` + `WorkspacePanesTests` passing.

## Remaining before merge

- Slice 7 above, then QA verification of Reset session on a remote Task and (if reachable) **New conversation** on a mini-owned Task (`scratch/qa.html`).
- Gate: the four acceptance commands below, then CI.

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
