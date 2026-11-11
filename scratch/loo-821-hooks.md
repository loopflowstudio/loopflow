# LOO-821: hooks call the Machine server

Built: the `start`, `compact`, `tool` and `stop` moments of
`lf __context-block` make one `session.boundary` request and open no store.
Described in `server/README.md` ("Turn and tool boundaries"),
`harness/README.md`, `docs/troubleshooting.md` and `docs/dev/server-first.md`.

Built: managed is decided by the command, never read back from
`agent_sessions.work_source` (Jack Heart: "the cli comamnd used should direct
down different codepaths"). `Cli::names_work` is true for `--task`, `--wave`
(Task and Wave conversations pass them) and a step of `lf task run`
(`journal::task_run_step`); `lf -b resume --task|--wave` passes it into
`run::resume`. The launch carries it as `AgentConfig::managed`; headless
spawns take it as an argument (`agent_process::spawn(…, managed)`), native
ones read `SessionCommand::managed`. A managed launch gets every hook, a steer
capability and the launch check (`steer_environment`, `require_server`). A
bare headless run gets two callbacks, start and compact, no capability and no
check: Claude and Codex through `ContextDelivery::hook_settings(managed)`,
OpenCode and Pi through `spec.managed` in their plugin source. Deleted:
`WorkSource::is_managed`, `PromptBuild::name_work`, the `work_source` reads in
`steer_environment` and `resolve_session_config`, and the Task-run step's
`declared` write in `ops/run.rs` (steps record `checkout` again, as on main).
The column is still written as before.

Not built, and why: a bare interactive launch (`lf -i`, `lf implement` at a
terminal, an agent started from a Desktop shell) keeps every hook and its
capability, with no launch check, as before this change. Built bare first,
then `transport_tests retained_hooks_survive_upgrade` failed, and the reason
is not the test: while an owner process holds the conversation, the boundary
hooks are what say it can take a reply (`record_native_turn`, `yielded`),
acknowledge input sent to it (`acknowledge_native_input`) and start a pending
Session move (`server/boundary.rs`). The whole `transport_tests` fixture is
`lf session serve -- lf -i -a claude : <prompt>` with no Task. Those hooks
can go when "Bare launches go direct" removes the owner; `native_args` and
`steer_environment` (the `interactive` column) are where.

The Graph marks its Task-run steps (`LF_GRAPH_STEP=task`, read once by
`journal::take_graph_step`); that mark, not the Session row, makes the step
managed.

Looked at and left alone:

- `moment` and `callback.hook_event_name` both describe the boundary. The
  values are `--moment` arguments of hooks installed in live conversations
  (`retained_hooks_survive_upgrade`); renaming breaks those.
- The server opens a store connection per boundary, as every served read and
  write does; `server-first.md` has that as its own open item.
- The server rewrites the delivery file after a checkout move and the hook
  rereads it; changing that is another Task.
- Pi's bash guard (`--moment tool-group`) stays on bare launches: it does not
  record, it joins the tool's process group to the watchdog that ends it with
  its provider (`agent_process::guard_tool_lifeline`).
- A Task-run step learns it is one from `LF_GRAPH_STEP=task`, not from a
  `--task` argument. Passing `--task` to the step would make `names_work`
  read only flags, but `--task` also drives cwd and checkout resolution and the
  Session's binding (`lf/commands/run.rs`, `bin/lf.rs`), which steps don't
  take today. The Graph spawning its step is the command; the mark is its
  argument.

Concept review with a person, November 11 (`scratch/concept-review.html`).
All three are answered below.

1. Which launches need the server. Shown the two rows where the built check
   and `server-first.md`'s managed rule differ (`lf implement` typed in a Task
   checkout; a subagent `lf research` started by a Task's agent), Jack Heart:
   "I think these are both ok", then "no i mean 1b -- i think its ok that
   these dont get hooks". 1b on the page: launches through Task and Wave
   commands and Desktop need the server; a plain `lf <skill>` in a Task
   checkout runs without one and still counts toward the Task. Told that a
   launch with no hooks needs its context in the launch prompt, Jack Heart:
   "these should be lighter weight." and "if its a subagent the agent driving
   can steer". Then: "hmm i think maybe we should have the context and
   post-compaction hooks", so context stays hook-delivered for every launch
   and nothing moved into the launch prompt. Then: "lets make sure to think
   about opencode and pi here too". Built as above.
2. A stale server passes the launch check (hello) and then answers no
   boundary (QA, beside installed 0.15.2). No server and a stale server leave
   the same conversation; only one refuses. Choices: as built, refuse both, or
   start both with one line at launch naming the fix.
   Answered, Jack Heart, November 11: "existing servers today shouldnt be
   designed around. just upgrade servers as necessary, use dev servers to
   test etc. in the future we should do somethign similar to waht rex server
   and client do and/or our desktop and server do for protocol negotiation".
   No change: no warning or refusal for an older server.
   Then: "avoid adding an additional handshake exchange, reuse desktops
   basically". The hook and launch check already send Desktop's `hello`
   (`revision`, `min_revision`); the server at 0.15.2 and this branch both
   speak revision 4, so the hello agrees and the missing operation shows up
   only as `invalid {kind}`.
3. Steer confirmation puts every steer through `uncertain` for a tool
   interval. Choices: keep it and show that state as `sent`, or have the hook
   send its deadline so a late server hands over nothing (no `.printed`, no
   `waiting`; a steer is lost only when a hook dies between read and print).
   "arrive twice" was said when the choice was lost-until-retried or twice.
   Then, Jack Heart, November 11: "No real opinion, worth following t3 code
   or rex's appraoches here". Rex has no steer concept: `terminal.write`
   "returns once the text is queued, without waiting for the program to read
   it". T3 Code (`pingdotgg/t3code` 25631c3c, `EffectOutbox.ts`,
   `EffectWorker.ts`): a steer is an outbox row sent through the provider's
   own channel (Codex `turn/steer`, Claude SDK streaming input, Pi RPC);
   confirmation is the call returning; failures retry up to 5 times, which
   can duplicate; a server restart cancels it silently. So T3 accepts
   duplicates while running. Kept as built (option a); no change.

Answered November 11, Jack Heart: "no. in 824 i explained that there is no
such thing other than one you start through a shell. shells in desktop should
be managed by the holder as well, but they should not be managed (and their
pty holding should be more generic than the lf session one)". A Desktop
conversation with no Task or Wave is an agent started from a Desktop shell, and
it is bare: the rule as built stands. Holding a PTY and being managed are
separate: LOO-824's holder holds Desktop shells too.

What the bare rule costs, found while building it (see `questions.md`):

- Native Pi reports its session file through `pi-event`'s `session_start`
  (`harness/pi.rs::observe_native`); `lf resume` continues from it. Bare
  interactive launches keep their hooks, so this still works. It is one more
  thing the hooks carry when they go.
  Outside lf, Pi resumes by session file or ID: `pi --continue` (latest in
  the directory), `pi --resume` (picker), `pi --session <path|id>`, and
  `pi --session-id <id>` names a new session up front ("Use exact project
  session ID, creating it if missing", `pi --help`). Launching interactive
  Pi with `--session-id`, as Claude gets `--session-id`, would let lf know
  the session without the report. Not tried.
- A correction turn a Graph resumes on its step is managed: it runs as a
  Task-run step, whatever the Session row says.

Concept review again, November 11, headless, after managed moved to the
command: the actions, the managed rule in `server/README.md` and
`server-first.md`, and the code agree (`Cli::names_work` →
`AgentConfig::managed`; no `is_managed` or `work_source` read left). Nothing
to change, no open choice. `scratch/concept-review.html` now shows the rule as
built.

LOO-824 (its concept review, Jack Heart: "managed should = held") will read
`AgentConfig::managed` and call `require_server` instead of keeping its own
rule; whichever branch lands second makes the switch.

Seen in the plugins, not changed here:

- A managed Pi Session polls `--moment tool` once a second while working: one
  `lf` process and one `session.boundary` each second (the Pi probe in
  `harness/context.rs` shows the `tool` calls).
- A managed OpenCode Session runs `lf __context-block` on every model request.
- `context_opencode.js` never resets `compacted`: after the first compaction
  every request sends `--moment compact`. Read in the source, not run.
- OpenCode and Pi put the block into the system prompt of each request
  (`input.system.push`, `sections.loopflow`), where Claude and Codex add it
  once per start or compaction.

Remaining:

- Demo with a real Claude turn in a Task Session. QA (`scratch/qa.html`) ran
  the same sequence with the branch CLI, a launchd server and a stand-in
  `claude` running the installed hooks: steer delivered and `emitted`, no hook
  holding the store, frozen and stopped server each one gap line, duplicate
  after a lost `.printed`. A real Claude login was not available. That QA
  predates the bare rule; its launch was interactive, which is unchanged.
- Bare interactive launches drop their turn hooks with "Bare launches go
  direct" (above).
- Gate: the suites under Checks marked gate.
- `limit` / `record-limit` still open the store, on a usage limit only.
- An interactive resume of an existing conversation (`lf resume`, Desktop
  reopening) runs no launch check: it gets every hook either way, and the
  resume path carries no command flag down to the native launch.
  QA, November 11: `lf -i --task <id> resume` with the server stopped ran
  the Task's conversation without starting it; a steer posted meanwhile
  waited and the start hook logged one gap. `resume_session_with_env`
  resolves its config with `Cli::default()` in the process the owner execs.

Choices taken (reversible):

- `session.boundary` under revision 4; hello `client.kind` is `"hook"`. An older
  server answers `invalid {kind}`, which the hook treats as a gap.
- Server answers `{steers: {batch, text}, waiting}`. The hook renders the block from checkout
  files; checkout relocation moved to the server, which rewrites the delivery
  file, and the hook rereads it after an answer.
- One two-second deadline for connect, hello, answer.
- Server failures (store, foreign conversation) answer `unavailable`; the hook's
  marker is the only gap memory. No per-Session gap state in the server.
- Steer confirmation (replaces deliver-on-answer). An answered batch stays
  `uncertain`; the hook prints it, appends its ID to `<delivery>.printed`, and
  the conversation's next request sends those IDs as `printed`; the server
  marks them `emitted` and returns every other `uncertain` batch of the
  Session older than `HOOK_DEADLINE` to pending (`confirm_session_steers`).
  No schema change: `uncertain`, `emitted` and `cancelled` already existed.
  A request moves the `.printed` file aside (`.printed.<pid>`) while it asks,
  so a hook printing beside it loses nothing; unanswered, it puts the IDs back.
- `waiting` in the answer: an earlier batch is unconfirmed and younger than
  the deadline. The hook then drops its seen marker like it does after a
  delivered batch or no answer, so the next tool boundary asks again. Without
  it a tool finishing within two seconds of a late answer would leave the
  steer until the next prompt.
- A `stop` confirms and returns batches to pending but delivers nothing; the
  next prompt's hook always asks.
- Upgrade: a store holding an old `uncertain` batch (an emitter that died
  between writing and marking, before this change) answers that steer once
  more at the Session's next boundary.
- `lf session steers` prints the server's connect error and "Steers wait and
  turns go unrecorded until it runs." when no server listens; the capability
  line reads "the Machine server has recorded no callback from this
  conversation". The gap line already named the server ("Loopflow server
  unavailable; check `lf server status` and the selected installation") and is
  unchanged.
- Fixtures that launch Task or Wave agents hold a
  `loopflow_test_support::MachineServer` for their home: the whole fixture in
  `session_lifecycle_tests`, `session_launch_contract`, `world_recipes` and
  `task_graph_launch_tests::GraphTask`, single tests elsewhere
  (`graph_tests`, `context_launch_tests`, `settings_launch_tests`,
  `session_cli_tests`, `land_tests`). Found by running every loopflow test
  with the supervisor call stubbed out.
- `context_block_tests::hook_after_repository_move…` is replaced by
  `server::boundary::…a_moved_checkouts_delivery_follows_the_repository`: a
  hook with no provider context no longer asks the server, and the store, not
  the hook's cwd, says where a repository moved.
- `stop`: planning refresh and pending move run on a server blocking worker,
  spawned before the answer; the move waits 2 s for the turn to end.
- PostToolUse / UserPromptSubmit hook timeouts 30 s → 10 s.

Deleted: the hook's store opens (`hook_store`, `acquire_planning`,
`deliver_steers`), `ContextMoment::Move`, `apply_session_move`,
`start_pending_move`, `wait_for_hook_exit`, the hook path's Work binding in
`bin/lf.rs`, and `linear_on_demand_native_hooks_share_freshness_across_processes`.
Turn-end planning refresh now has no Linear-fixture test;
`server::boundary::after_stop` calls `sync_repository` directly.

Boundary queries (November 11; Jack Heart: "we should make sure this is
going to scale to a lot of concurrent work"). Every statement a start,
prompt, tool and turn-end boundary runs was traced and planned against a copy
of the real store (10,269 batches, 40,119 deliveries, 3,019 steers, 1,094
Tasks). Scans fixed:

- `steer_batches` by Session (recover, confirm, waiting, reconcile's prepared
  cancel): `steer_batches_session(session_id,evidence,created_at)`.
- `steer_deliveries` by batch (cancel, excerpt): `steer_deliveries_batch`.
- reconcile's applicable steers read every steer:
  `steers_session`, `steers_repo` (Task and Wave already indexed).
- reconcile's cancelled-delivery delete read every cancelled batch: scoped to
  the Session.
- reconcile's checkout membership read every Task, twice: `session_tasks`
  now finds candidates by the cwd's ancestors through `idx_tasks_worktree`,
  and `member_tasks` is materialized. Reconcile 1.7 → 0.23 ms.
- Left: the history read at a conversation switch (`resume_reference`) plans
  wave/task scans in branches its NULL filters never take; 8 ms on the
  Session with the most events.

No write lock when there is nothing to write: `recover_prepared_steers` with
no prepared batch, `confirm_session_steers` with nothing printed and nothing
past the deadline, `with_provider_context` on a turn in the selected,
activated, already observed conversation, `emit_session_steers` with no
undelivered applicable steer (its reconcile was rolled back in that case
anyway). Test:
`a_boundary_with_nothing_to_settle_does_not_wait_for_another_sessions_write`.

50 concurrent Sessions, each on one of the store's 50 most recent Tasks and
its Wave, each request opening its own store connection as the server does
(`scratch/qa/fifty_sessions.rs`); debug build, SQLite at opt-level 2, load
average 80–250 on 16 cores from other work:

| | before | after |
|---|---|---|
| 1 Session, tool | 21 ms | 6 ms |
| 50, tool ×20 each: p50 / p95 / p99 / max | 186 ms / 4.0 s / 7.8 s / 13.6 s | 641 ms / 1.05 s / 1.19 s / 1.4 s |
| 50, prompt with a steer: p50 / p95 | 1.1 s / 4.3 s | 559 ms / 1.66 s |
| 50, turn end: p50 / p95 | 1.0 s / 1.39 s | 625 ms / 800 ms |
| mean store open in the tool burst | 26 ms | 550 ms |

Before, boundaries queued on the write lock (p99 far past the hook's 2 s
deadline: gap lines). After, they run side by side and the store open
dominates: each open validates the whole migration history (5 ms alone).
Added to `server-first.md` Open; not changed here.

QA fix (November 11, after managed moved to the command):
`SessionCommand::managed()` read `launch`, which `build_session_command`
fills only for Codex and OpenCode, so `lf -i --task X -a claude` (and Pi) ran
no server check: live, its start hook gapped and the server stayed down. The
command now carries `managed` itself. Test:
`session_steers::an_interactive_task_launch_with_no_machine_server_fails_naming_it`
(fails without the fix).

QA fixes (November 11; both were on main before this branch):

- A new Session on a Task got every earlier Task steer again. `reconcile`
  (`store/sqlite/steers.rs`) now skips a shared steer saved before the
  Session started that another Session was already handed (`emitted`); a
  steer saved after it started, or still pending for no Session, arrives.
  Its lookup is indexed by `steer_deliveries_steer`
  (`migrations/steer_delivery_lookups.sql`, this branch's one migration).
- OpenCode wrote a new `context-manifest.md` on every model step (20 files in
  40 s). The manifest is named by its content hash
  (`context_block::write_manifest`), so an unchanged checkout reuses one file
  on every harness.
- `store::sqlite::steers::tests::a_new_task_session_skips_steers_an_earlier_session_read`,
  `context_block::tests::repeated_requests_over_unchanged_files_write_one_manifest` — pass

Checks (November 11, managed by command and boundary queries):

- `--lib -- server::boundary:: store::sqlite::steers store::sqlite::provider_transitions store::sqlite::task_work` — 36 pass, then the two new tests pass
- `--test integration -- session_steers:: context_launch_tests:: settings_launch_tests::` — 35 pass
- `--lib -- task_graph_launch_tests::a_task_run_takes harness::agent_process:: harness::context:: lf::tests::` — 66 pass
- `--lib -- store::migrations::` — 129 pass; `scripts/check_migrations.py` — only `unmerged: steer_delivery_lookups.sql`
- clippy — pass
- Compress: `confirm_session_steers` drops its in-transaction authority pre-read (the closing read still checks and rolls back); `--lib -- store::sqlite::steers server::boundary` — 27 pass
- gate: `transport_tests`, `binary(two_people)`, `world_recipes::`, `session_lifecycle_tests`, Swift `ServerProtocolTests`

Checks (November 11, on the bare rule):

- After merging main (#1881 moved server fixtures under `rust/loopflow/tests/`): `cargo test -p loopflow --lib server::schema` — 7 pass; Swift test paths already point at the new directory
- `cargo test -p loopflow --lib -- harness::context:: context_block::tests::a_bare harness::agent_process:: store::sqlite::steers` — 43 pass (bare hook settings; OpenCode and headless Pi probes under node)
- `cargo test -p loopflow --test integration -- session_steers:: documented_commands::` — 9 pass (new `a_bare_skill_in_a_task_checkout_gets_context_hooks_and_asks_no_server`; the managed ones launch with `--task`)
- `task_graph_launch_tests::a_task_run_takes_its_waves_agent…` — pass: the step's Session is `declared` and its settings carry `Stop`
- `cargo test -p loopflow --test transport_tests -- hook continue_session resumed_session unanswering` — 10 pass
- `--test integration -- context_block_tests:: context_launch_tests:: global_commands:: process_ownership_tests:: session_cli_tests:: settings_launch_tests::` — 52 pass, run before interactive launches got their hooks back; gate
- `--lib -- harness:: context_block:: lf::commands::context server::boundary process::runtime::tests ops::run:: store::sqlite::steers task_graph_launch_tests:: graph_tests::` — 342 pass, 4 fail in a 20-minute run, before the same change: two `harness::pi_tests` (pass alone and in a 93-test `harness::` run) and two that only met the lock those poisoned; gate
- clippy — pass
- Swift `ServerProtocolTests`, the rest of `transport_tests`, `binary(two_people)`, `world_recipes::`, `session_lifecycle_tests`, `served_reads` with a resolved `TMPDIR` — gate

Checks (November 10, on the launch check, before the v0.15.3 merge, which
changed only release files):

- `cargo test -p loopflow --lib -- server::boundary server::client lf::commands::context` — 18 pass
- `session_steers::` — 5 pass (new `a_task_launch_with_no_machine_server_fails_naming_it`); `public_context_capability…` failed once at 10 s in a 17-test run and passed alone and in the last run: load
- One Task-launch test from each converted suite, `binary(two_people)`, `world_recipes::`, `documented_commands::` — pass (51 in the last run)
- `launchctl list | grep com.loopflow` labels before and after those runs — the same
- Every loopflow test with `readiness_command` swapped for a marker file (3671 run): no test reaches the supervisor after the `two_people` fixture got its servers. Failures in that run that passed alone on the real code: `claude_body_activity…`, `real_process_cpu_sampling…`, `reconnect_preserves_live_credential…`, `an_interrupt_ends_the_running_agent_step…`, `release_run_repairs_failed_checks…`, `unanswering_superseded_owner…`
- `served_reads screen_reads_are_compact_passive_and_match_cli` fails with or without the launch check (`tasks.items` empty); `scripts/testing.md` says to run `served_reads` with a resolved `TMPDIR`, not tried — gate
- clippy — pass
- Compress: `transport_tests` uses the shared `MachineServer`;
  `retained_hooks_survive_upgrade`, `native_drafts_remain_editable…` — pass
- `cargo test -p loopflow --test integration dto_fixtures` — 50 pass (before this pass)
- Swift `ServerProtocolTests` (`hookBoundarySharesTheRustServerContract`) — gate (`--swift`)
- `transport_tests` hook tests other than the two `continue_session…` ones, `harness::` — gate
