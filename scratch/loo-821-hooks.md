# LOO-821: hooks call the Machine server

Built: the `start`, `compact`, `tool` and `stop` moments of
`lf __context-block` make one `session.boundary` request and open no store.
Described in `server/README.md` ("Turn and tool boundaries"),
`harness/README.md`, `docs/troubleshooting.md` and `docs/dev/server-first.md`.

Built: the managed rule, `session.rs::WorkSource::is_managed` (`declared` or
`bound`). A managed launch gets every hook, a steer capability and the launch
check (`harness/agent_process.rs::steer_environment`, `require_server`). A
bare headless run gets two callbacks, start and compact, no capability and no
check: Claude and Codex through `ContextDelivery::hook_settings(managed)`,
OpenCode and Pi through `spec.managed` in their plugin source. The launch
config carries it as `AgentConfig::managed`, set where the command names its
Work (`lf/commands/run.rs::PromptBuild::name_work`) and from the Session row
on resume (`agent/session.rs::resolve_session_config`).

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

A step of `lf task run` is now recorded `declared`, not `checkout`: the Graph
marks its Task-run steps (`LF_GRAPH_STEP=task`, read once by
`journal::take_graph_step`), and `ops/run.rs::resolve_execution_binding`
names the Task for them. Before this, this Task's own `implement` step was
`checkout` in the store, the same as `lf implement` typed in the checkout.

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
  its provider (`agent_process::guard_tool_lifeline`). The comment above it in
  `context_pi.mjs` ("records the tool boundary") is stale.

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
- A headless Session started as a `lf task run` step before this change is
  `checkout` in the store; a correction turn resumed on it is bare.
  Jack Heart, November 11: "Thats fine".

LOO-824 (its concept review, Jack Heart: "managed should = held") will call
`WorkSource::is_managed` and `require_server` instead of keeping its own
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
