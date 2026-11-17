# LOO-932: Start lands in the conversation

Task brief: `lf task status LOO-932`. Sibling briefs and the shared model:
`69d8b36466:scratch/task-briefs.md` (LOO-906 branch, reference only).

## Demo

Desktop, local Machine, Loopflow repository. Type a request in New Task and
click Start. In that same frame the sheet is gone and the Task's workspace shows
the words you typed in its conversation pane. Lines appear under them as setup
finishes — `Checkout ready  0.2 s`, `Graph code started` — then the provider's
prompt, which takes typing. The navigator row is in Working the whole time;
"Reading workflow", "Restoring workspace" and "Not started / Start" never show.
`uv run python scripts/benchmarks/desktop-performance/timings.py` shows the
Start → typing sample with its server creation steps underneath.

## Baseline (run, 2026-11-16, `f89e9b5429:scratch/performance-baseline.md`)

- New Task Start → typing: 23,781.5 ms usable, 95.8 ms acknowledged, n=1,
  12 overlapping main-thread stalls; load average ~49; server was `470da475a`.
  Raw: `~/.lf/performance/live/20261116-start-complete/`.
- `creation_step` spans were compiled but not live-validated; that server
  logged "attempt to write a readonly database" while recording the launch.
- No remote-Machine Start → typing exists; `timings.py` labels everything local.

Target: server reply under 300 ms; Start → typing on a local Machine with a
ready checkout = provider boot + under 1 s; the sheet closes in the click.

## Today (main + LOO-916)

Server, inside one `task.create` request (`engine/creation.rs::create`):
1. `ops/task.rs::task_create`: `planning_sync::acquire_repository` (network for
   Git planning), `adoption::save` (Task + receipt `filed`, one transaction,
   `sqlite/children.rs::promote_task_prepared`), `sync_after_save` (Linear/Git
   exports, network, inline).
2. `primary::ensure_task`: primary Session row (cwd = main repo), then
   `prepare` → `admit_workspace` → `task_checkout` → `prepare_task`:
   `fetch_task_refs` (fetch all branches), `resolve_upstream_base` (second
   fetch), `git worktree add`, `assert_assignee` (Linear `viewer_id`), then
   `admit_session_workspace` moves the Session's cwd.
3. `launch::start_creation` spawns `lf task run --json` (setsid) and polls a
   report file every 50 ms. The child takes up the Graph, starts the held
   conversation (`start_in_background` → `transport::start_held` → holder runs
   `lf session resume`, which execs `lf -i …`), spawns `lf graph run`, and
   polls every 50 ms, no deadline, until the GraphProcess registers.
4. Receipt → `admitted`; reply `{task, graph, session_id, milestone}`.

Client after the reply (`WorkModel.createTask`, `openTaskLink`,
`openLinkedTask`): `plan.list` for the Task, workspace placement with a
`desktop.layout` read ("Restoring workspace", `SessionsView.swift:1843`),
`session.resume --task` prepare, a second `plan.list`, then the pane spawns its
`lf session resume ID --task X` relay. Before that frame `taskWork[id]` is empty
("Reading workflow…", `TaskWorkView.swift:441`), and `task(id:)` takes the
roadmap row first (`WorkModel.swift:2904`), which is `unstarted` until the child
takes up the Graph ("Not started"/`TaskStartOffer`, `TaskWorkView.swift:590`).

`lf` starts on the path: `lf task run`, held `lf session resume`, its exec into
`lf -i`, the pane relay, plus `lf holder run` when cold. `lf graph run` sits in
the reply path.

Repeat with the same request ID (`sqlite/creation.rs::recovery_in`): `filed`
+run → Continue (redoes the launch), `run_requested` → Inspect "Task created;
start unconfirmed", `admitted`/`started` → Complete.

## Approach

**Admission is one transaction, and the conversation does the setup.**

`task.create {run: true}` writes, in one IMMEDIATE transaction: the Task, its
claim by this person on this Machine, the Graph taken up at its first step
(`task_work::take_up_task_graph`), the primary Session (cwd = main repo, request
= the typed words, `WorkSource::Declared`) and the receipt at `admitted` with
Task and Session IDs. No network, no Git beyond resolving the repository and
the Graph file. The Task is never `unstarted` in any reading, so "Not started"
cannot appear and no row-ranking fix is needed for it.

Then the server starts the held conversation (`transport::start_held` of
`lf session resume <id> --task X`) and replies. The answer carries the receipt,
the prepared `SessionRecord` (with `open_argv`) and the Task's `TaskOverview`
reading taken after commit; a repeat reads both again, the stored receipt does
not hold them.

After replying, a server continuation on `journal::request::spawn_blocking`
under the request's trace runs `planning_sync::acquire_repository`,
`sync_after_save`, then `assert_assignee`. Failures are a Task event and a log
line; they never touch the conversation.

The held `lf session resume` runs setup before the provider, printing one line
per step into the terminal:
1. **Checkout**: `admit_workspace` takes the ready checkout (below), else
   `git worktree add` from local `origin/<default>`. No fetch, no PR lookup (a
   new branch has none), no Linear. Then `admit_session_workspace`.
2. **Graph**: when the Task's Graph is taken up and has no run, start it the way
   `start_task_run` does (`lf --task X graph run`, detached, not awaited). The
   Graph's own `launch::report` moves the receipt to `started`.
3. **Provider**: runs in this process; no exec into `lf -i`.

Each step reads recorded state first, so a restarted conversation resumes setup
where it stopped and never launches a second Graph. A step that fails prints
the error in the pane and the provider still starts when the checkout exists.

**Milestones**: `filed` (run=false), `admitted` (Task + Session recorded; the
conversation owns the rest), `started` (Graph launch recorded). `admitted` and
`started` repeat as Complete. `run_requested` is no longer written; released
rows that hold it stay Inspect.

**Ready checkout** per repository on the Machine that runs Tasks:
`<parent>/<repo>.ready`, a detached worktree. Taking it: `git worktree move` to
the planned path, `git switch -c <branch> origin/<default>` (local ref; cost is
the diff since it was made). After a take, and on the first Start in a
repository without one, the server continuation runs `git fetch origin
<default>` and `git worktree add --detach` for the next. Base = `origin/main` as
of the last background fetch; the Task's first sync catches up.

**Desktop**: the sheet closes on click into the Task workspace, showing the
words (`StartingTaskView` shape from LOO-906, keyed by request ID, no
`PendingOperation`). On the answer, seed `taskWork[id]` from its reading, place
the pane from its `SessionRecord`, and skip `plan.list` ×2, the `session.resume`
prepare and the `desktop.layout` read for a Task this window created
(`CreationRecovery.shared.requests(for: .task(id))` is non-empty). The pane's
relay attaches to the already-held terminal.

**Phone**: on the answer, push `TaskWorkspaceView` with the receipt's Session
and `session.attach` at once; drop the `task:<id>` view-plan read before it.

**Journal**: `timings.py` reads `task_creation`/`creation_step` spans from
`logs/lf.log` by request ID (the interaction observation ID) and nests them
under the Start sample; setup lines in the conversation log their own spans
(`conversation_setup` step=checkout|graph; the provider's start is the rest of
the sample). Perf records the Machine
kind (local, remote) on `start_task`; the report splits by it.

## Risks checked

- Session before checkout: `LfSession.cwd` is required but not a worktree;
  today the row is written at the main repo and moved by
  `admit_session_workspace` (`sessions.rs:1453`). Safe to record first.
- Repeat during setup: today `filed`+run would redo the launch. Writing
  `admitted` in the admission transaction makes every repeat Complete.
- `start_in_background` skips a Session whose cwd ≠ worktree; the new path
  starts the held conversation itself, so that guard is not on it.
- `serve_locked` execs `lf -i` so no connect sits beside the conversation, and
  passes the launch lock across exec (`LAUNCH_LOCK_ENV`). Running the provider
  in-process keeps both properties with the lock simply held.
- Branch-name collision is checked against refs from the last fetch; a
  collision surfaces at first push. Accepted.
- Cleanup and `<repo>.ready`: listings drop it, so `wt prune --all` keeps it
  (tested). A repository with submodules refuses `git worktree move`; the take
  then falls back to `worktree add`.

## Alternatives

- Server continuation does checkout and Graph start and the conversation waits
  on store events: two owners of ordering, a store-polling wait in the
  terminal, and setup progress needs a channel into the pane. Rejected.
- LOO-906's `operation.submit` admission (envelope saved, no IDs): replies
  early but the Session ID still comes at the end, so typing is no sooner.
- Keep `lf task run` and make it faster: the polling and two process starts
  are the cost. Rejected.

## Delete — do not maintain

- `engine/launch.rs::start_creation` and its report-file polling, if creation
  is its only caller; creation's `lf task run` argv in `creation::create`.
- `run_requested` writes (`advance_creation(id, "run_requested")`) and
  `record_creation_launch`'s `WHERE milestone='run_requested'` gate (gate on
  `admitted`); the Inspect text stays for released rows.
- Done: `serve_locked`'s exec and its carriers (list under Status).
- Network steps inside creation's checkout: `fetch_task_refs` and the base fetch
  in `resolve_upstream_base` for a new branch, `assert_assignee` inside
  `prepare_task` on this path.
- Done: Desktop's `"new-task"` Perf key (the click's request ID is the
  interaction's ID and the pending key), the sheet's wait, post-reply `plan.list`,
  `prepareTaskSession` and the `--explain` fallback for a created Task.
- Phone: `createdTask`'s view-plan read before navigation (done for a first
  Start; kept for a request recovered after a restart, whose receipt has no Session).

Behavior that must survive: `lf task run` on an existing Task (still starts the
conversation beside its Graph), `-b`/approve creation (no Session; keeps
launching `lf --batch task run`), adoption with `--session`/`--worktree`,
`CreationRecoveryView` resend for a lost answer, content-mismatch Conflict.

## Forbidden

A second creation path for Desktop beside the CLI; a client-side Task row
fabricated from the request; a polling wait for checkout inside the terminal;
`lf task run` on creation's answer path. (Setup starting the Graph through a
detached `lf task run` is the one Task-run carrier, not a fallback: its
`start_in_background` finds the held conversation opened and skips.)

## Status (2026-11-16)

Slice 1 built. `task.create {run}` files Task, claim, Graph take-up, primary
Session and the `admitted` receipt in `promote_task_prepared`'s IMMEDIATE
transaction (`ops/task.rs::admit_task` → `TaskCreationRecord.admission`), holds
the conversation, answers `TaskCreated` (receipt + `SessionRecord` read with
`surface`, no workspace preparation + `TaskOverview`), then runs `sync_admitted`
on `journal::request::spawn_blocking` (`creation::After`; dropping it detaches,
the CLI joins). The held `lf session resume` prints the request,
`Checkout ready  N s`, `Graph <name> started`, then execs the provider.
Differences from the approach above:
- Setup launches `lf task run <ID>` detached (the carrier that retries and
  settles completion), not `lf graph run`, and records it with
  `record_setup_launch` (`admitted` → `started`). It finds its creation via
  `admitted_task_creation(session)` (migration `creation_receipt_sessions.sql`).
- The conversation is held even with no agent selected: setup needs none.
- A viewer of a held terminal (`connect`, `held_elsewhere`) skips setup.
- `-b` creation keeps `run_requested` + `start_creation` (no conversation).
- An adopted Task with conversations but no primary gets its ranked
  conversation (`task_conversation`) chosen inside the transaction.
- A released `filed`+run row for a conversation-admitting create answers
  `outcome_unknown` instead of continuing.
- Setup's "start the Graph" obligation lives on the receipt (`admitted` +
  `session_id`). Nothing prunes `creation_receipts`; whatever later expires
  request IDs must keep `admitted` rows.

Concept review of dde1d32a15: the model holds (one transaction; a crash after
commit and before `hold_conversation` repeats as Complete and the conversation
sets up when opened). Its one cleanup is done: `ops/task.rs::creation_record`
builds the `TaskCreationRecord` for both the adopted and new branches of
`file_task_in`. `Receipt`/`Admit` stay as the caller's input (graph name,
request); `TaskCreationRecord`/`TaskAdmission` are the resolved rows.

Slice 2, Desktop half built. Start closes the sheet in the click into
`StartingTaskView` (`navigation.startingTask`, keyed by request ID; a failure
before filing shows there with Try again). `WorkModel.openCreatedTask` seeds
`taskWork` from `TaskCreated.overview`, starts the workspace's layout without a
`desktop.layout` read (`SessionsWorkspace.startNewLayout`) and installs
`TaskCreated.session` as prepared (`installConversation`, `preparedIntent:
.start`). The position line says
"Starting <graph>" for a claimed Task at `start`. `WorkDestinationTests` checks
the request log after Start holds only `task.create`.

After the answer, no Try again (1f8eb5476e): `createTask` throws only before
filing (`StartingTaskView` keeps the words and Try again); an opening failure
lands on the selected Task's `conversationError`
(`WorkDestinationTests`: one `task.create`, `startingTask` nil).

The answer carries the Task's row (`TaskCreateAnswer.row`, Rust
`lf::commands::task_create`, read by `waves::task_row`; Swift
`TaskCreated.row`). `openCreatedTask` selects from planning's row, else the
answer's (retained evidence, "Updating…"), with no Work checkpoint wait, and
never throws; `StartingTask.filed`, Close and "planning does not show it yet"
are gone, and only `StartingTaskView.start` closes the palette. A real answer
showed the admitted Task `unstarted`/Backlog: `task_state_sql` called a Task
unstarted until it had a checkout. Unstarted now means no checkout and no
taken-up Graph (every take-up is a start), so the admitted Task is Working.

Slice 2, phone half built. A first Start is `startTask`:
`CreationSubmissions.submit` saves, sends once and decodes `TaskCreated`
(`MobileTaskStart`); the model keeps its `MobileTask` (ID, identifier, title,
primary Session; also built from a reading's row in the Task list) in memory,
so Open Task reads nothing and `SessionLink` attaches on the first terminal
fit. "Check status" is `recoverTask` (`reconcile`); a request recovered after
a restart still reads `task:<id>` through `createdTask`, since its receipt has
no Session. `SessionLink` takes the Session ID; its no-Session branch is gone
(`TaskWorkspaceView` shows Changes then). `MobileTaskCreationTests` checks
Start's log is `task.create` alone and the opening names the answer's Session
(fixture `task_create_receipt.json`). Not covered by a test: the
`RemoteMobileClient` wire half (no fake Machine in the iOS suite).

Concept review of 4d6ec717da: the phone model holds (`MobileTask` is the one
value both Start and the Task list open; recovery alone reads `task:<id>`).

The first send reads "Starting…" (`MobileTaskCreation.starting`); recovery
reads "Checking your Task…".

Concept review of 1f8eb5476e: the split holds (Try again only before filing;
after the answer the failure is the workspace's).

Concept review of 91a35e2430: the row closes the gap without a new concept.
`openCreatedTask` reuses `selectedTaskEvidence` (set by `openTaskDestination`,
kept by `setSelection` through `task(id:)`), so the answer's row is the same
retained evidence a lookup leaves, replaced when the roadmap lists the Task.
The row read (`task_create::answer` → `waves::task_row`) opens a second
store handle and a tokio runtime, and runs `SharedTaskReads::read` (the whole
repository's selection reading) on the answer path, outside any
`creation_step` span.

Start's request ID is made at the click (`StartingTaskView.start`), passed
to `createTask(requestID:)`, and names both the Perf pending key and the
journal Interaction (`Interaction(id:)`; `Perf.begin` uses a UUID-shaped
Perf ID as the Interaction's). The row read is `creation_step step=row` under
a `task_creation request_id=…` span (`task_create::answer(id, …)`).

Slice 3 built. Setup's checkout is `ops::task::admitted_checkout`
(`TaskProcessOptions.from_last_fetch`: no `fetch_task_refs`, base from
`local_upstream_base`, no `assert_assignee`). `restore_task_checkout` takes
`git::worktrees::ready_checkout` for any new local branch
(`take_ready_checkout`: `worktree move --force`, `switch --no-track -c`;
falls back to `worktree add`, Git refuses a checkout still initializing).
Setup then calls `prepare_ready_checkout`, a `setsid` shell that backgrounds
`git worktree add --detach <ready> <integration base>`. `sync_admitted` ends
with `fetch_tracking_ref` of the default branch. `list_porcelain` drops the
detached entry at the ready path, so `wt list`, placement inference and
`wt prune` never see it. Replacement lives in setup, not the continuation:
the continuation can run before the take (or before the conversation is ever
opened) and would find the old one still there.

Synced with origin/main (#1953, #1955) at 3ced22521; main dropped
`SessionRecord.blocked`, so `task_create_receipt.json` lost it too.

`-a` built: `TaskCreation.agent` rides the held argv (`lf --agent X session
resume ID`, `primary::hold_task_conversation`), and setup's `lf task run` takes
the held process's command-line agent. The conversation already ran X: the
Session row's provider comes from `resolve_task_agent`, which reads the
invocation. Only the Graph lost it. Desktop's served `task.create` has no agent
field and sends none.

The `task_create_` fixtures registered a keepalive launchd server per temp home
(any launch without a server runs `lf install server`); 55 had piled up. Each
fixture that starts a Task now holds `Service`, whose drop waits for the
store's unfinished processes except `lf holder run`.

Slice 5's local half built. lf.log's filter (`warn,loopflow::server=info`) had
dropped every `creation_step` line, which is why the baseline never saw them; it
now keeps INFO for `engine::creation`, `human_session::setup` and
`lf::commands::task_create`. Setup's steps are `conversation_setup{request_id,
step}` with "Setup step finished elapsed_ms=". `timings.py` reads
`<home>/logs/lf.log` into each attempt's `machine_steps` (by lower-case request
ID) and per-group medians, and keys groups by `context.machine`; Desktop's
Start (both scenarios) records `machine: local|remote` from
`WorkModel.machineKind`. No `provider` step: lf cannot see when the provider
takes typing; the remainder of the sample is it.

Exec cost, one Start on a scratch home (debug build, load ≈ 50): checkout
1.73 s, graph 0.58 s; `fresh_launch_selected` → the exec'd `-i`'s
`prompt_begin` 0.58 s; prompt build 1.49 s; prompt end → provider exec 0.84 s.
Slice 4 removes the 0.58 s; prompt build is the larger cost.

Concept review of e01c813a0a found the ready checkout's owner split (only
setup replaced it, so a backlog `lf task run` that took it left none) and a
hand-deleted one never returning (its registration made plain `worktree add`
exit 128). Fixed: a successful `take_ready_checkout` calls
`prepare_ready_checkout`, which runs `worktree add --force --detach` (one
`--force` still refuses a locked, mid-checkout registration); setup's call
stays for a repository with none. The ready-checkout fixture deletes it by hand
and checks the next Start makes another.

Jack Heart on the one-start count: "I guess this is fine, im not sure what the
one lf start goal is about". Slice 4's test is "the provider's parent is the
held `session resume`", not a count of `lf` starts.

Concept review of 1122563065 and 3d582e361b settled the next pass; built:

- **One timing target.** `task_creation`, `creation_step` and
  `conversation_setup` take `journal::START_TIMING_TARGET`
  (`loopflow::start_timing`); lf.log's filter names it instead of three module
  paths.
- **Slice 4.** `open`, `open_without_replacing` and `fork` return an `Opening`
  (`Done(SessionRecord)` or `Launch(ConversationLaunch)`), `new_thread` returns
  its launch beside the Session, and `lf session resume|new` hands a launch back
  to `bin/lf.rs::launch_conversation`. That builds a `Cli` (no Clap parse:
  `-i`, the agent, `Inline` or `Skill External`, `prepared_capture`,
  `launch_lock`), selects the agent on its own thread, enters the cwd, sets
  `LF_AS`/`LF_WAVE_ID` as `-i`'s placement did, and runs the same
  `resolve_cli_target` → `execute_target`/`execute_command`. The capture rides
  `Cli.prepared_capture` into `begin_process_runtime`; the lock rides
  `Cli.launch_lock` → `AgentConfig.launch_lock` (`LaunchLock`, a take-once
  shared slot) into `launch_session`. Setup's Graph takes `cli.agent` through
  `open(…, agent, …)`.
- **Deleted:** `serve_locked`, `conversation_launch_args`, `pass_launch_lock`,
  `LAUNCH_LOCK_ENV` and its `env_remove`s, `inherited_launch_lock` and its
  fallback, `finish_before_exec`, `prepared_artifact_key`, `PREPARED_CAPTURE_ENV`
  (also Swift's env list), and `active_session_token`. Its only reader gone,
  `HUMAN_SESSION_ENV`, `HumanSessionToken` and `session_token` went too, with
  the docs/dev/environment.md row; the launch carries no token.
- **What the test checks.** In a held terminal the provider execs over its
  launcher (`exec_native`), so a held claude's parent is `lf holder run`, never
  the `session resume`. The creation test instead checks the provider's PID has
  exactly one `kind='lf'` LfProcess and it is `session resume <id>` (the exec
  path recorded two). The plain resume (`transport_tests::
  piped_resume_json_runs_here_and_returns_once`) runs unheld, and its provider's
  parent is the `resume <id>` that opened it, not `lf -i`.
- `session_cli_tests::boundary_launch_…` lost its block that made an `LF_BIN`
  reject the exec'd `lf -i`: no exec remains to reject.
- Two failures that predated this pass (checked on 5cf51f1eb4), fixed:
  `promotion_explicit_sources_with_run_keep_the_selected_work` (`--worktree`
  adoption made a new conversation; admission now takes the checkout's open
  one, `adoption::Source::conversation`), and
  `served_task_create_survives_a_dropped_connection_and_server_restart`
  (compared whole answers, whose `row` reading time differs; it compares
  receipts now, as `task_create_tests::receipt` does).

Remaining: slice 5's remote sample (qa).

## Slices

1. **Admission + conversation setup (Rust).** One transaction, held
   conversation started before reply, continuation for planning sync and
   assignee, setup steps in `lf session resume`, milestone rules. Test: served
   `task.create` with a fake `git` that blocks on `fetch` and a planning export
   that blocks — the answer arrives with Task, Session, `SessionRecord` and a
   Working reading; the conversation reaches the stub provider; the Graph
   registers once; a repeat with the same request ID mid-setup returns the same
   IDs and no second Graph.
2. **Desktop and phone land in the conversation.** Sheet closes on click; seed
   from the answer; no follow-up reads for a created Task. Tests in
   `WorkDestinationTests` assert the request log after Start holds only
   `task.create` (+`machine.self`), no `plan.list`/`session.resume`/
   `desktop.layout`, and none of the three labels render.
3. **Ready checkout.** Make, take, replace; check cleanup leaves it alone.
   Test: second Start in a fixture repo moves the ready worktree into place
   (same inode), no `worktree add` on the request, a new `.ready` appears.
4. **Provider in-process.** Drop the exec (shape in Status). Test: the stub
   provider's parent is the held `session resume`, on a creation Start and on
   a plain resume of a prepared Session; the `-a` fixture stays green.
5. **Journal.** Spans in the report, Machine kind split; record Start → typing
   on the live Machine and on a remote Machine.

## Checks

- `loopflow-execution --lib git::worktrees::` 22, `ops::task::tests::` 33 passed (slice 3).
- After slice 4: `cargo test -p loopflow --test integration task_create_` 10 passed; `ops::human_session::` 45 passed; nextest `context_launch_tests:: | settings_launch_tests:: | session_lifecycle_tests | session_launch_contract | session_cli_tests:: | task_create_ | adoption::tests:: | piped_resume_json…` 140/141 passed, the one failure (`served_task_create_survives…`) fixed and passed alone; Swift Mac filter (+`ProcessEnvironment`) 153 passed; clippy clean. A run beside another checkout's compile timed out `codex_startup_failures_…` (30 s deadline) and two `settings_launch_tests`; all passed in the unloaded rerun. Gate: `binary(served_reads)` whole, `binary(transport_tests)`.
- `swift test --package-path swift --filter 'WorkDestinationTests|NewTaskTests|MobileTaskCreationTests|DTOFixtureTests|LaunchJournalTests|InteractionTests'` — 152 passed (Mac).
- `cargo test -p loopflow --test integration dto_fixtures::task_create` and `task_create_tests::task_create_without_recovery_keeps_the_existing_receipt_and_exit_status` (real row is Working) — passed; `cargo test -p loopflow-storage --lib` 365 passed; `loopflow-execution --lib store::tests::` 47 passed; `server::schema::tests::` 8 passed; clippy clean. Gate: `binary(two_people)`, `test(performance_tests::)`, `test(ops::task::tests::)` for the Task state SQL.
- `uv run python scripts/test_ios.py -only-testing:LoopflowiOSTests/MobileTaskCreationTests` — 21 passed; full iOS suite at gate.
- `uv run pytest python/tests/test_desktop_timings.py` — 16 passed (nesting and Machine split).
- `cargo test -p loopflow --test integration task_create_` — 9 passed after the sync, `-a` and timing assertions; no launchd server left behind. Swift Mac filter (with `InteractionTests`) 153 passed; clippy clean.
- Live Start → typing, local and remote Machine — qa (remote needs a rented or
  tailnet Machine; not a gate requirement).

## Done when

Clicking Start closes the sheet into the conversation in the click, the words
and setup lines show, and the provider takes typing with none of the three
labels on the way; a repeat request returns the same Task; the report shows
Start → typing with server steps for a local and a remote Machine.

## Docs (written ahead of the code)

`swift/README.md` (New Task → conversation), `swift/LoopflowiOS/README.md`,
`docs/work.md` (milestones, setup lines, base, `<repo>.ready`),
`docs/troubleshooting.md` (receipts, "Setup failed in a new Task's
conversation"), `docs/dev/server.md`, `rust/loopflow/src/server/README.md`
(answer shape, milestone table), `tests/fixtures/dto/README.md`,
`scripts/benchmarks/desktop-performance/README.md` (spans, Machine kind).
Docs promise: `lf wt prune` (even `--all`) keeps `<repo>.ready`; deleting it by
hand is safe. Both hold after slice 3.
