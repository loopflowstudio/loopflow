# LOO-821: hooks call the Machine server

Built: the `start`, `compact`, `tool` and `stop` moments of
`lf __context-block` make one `session.boundary` request and open no store.
Described in `server/README.md` ("Turn and tool boundaries"),
`harness/README.md`, `docs/troubleshooting.md` and `docs/dev/server-first.md`.

Built: a Task or Wave launch starts its agent only beside the Machine server
(`harness/agent_process.rs::require_server_for_work`; words and the assumption
taken in `questions.md`). Described in `server/README.md` and
`docs/troubleshooting.md`.

Looked at and left alone:

- `moment` and `callback.hook_event_name` both describe the boundary. The
  values are `--moment` arguments of hooks installed in live conversations
  (`retained_hooks_survive_upgrade`); renaming breaks those.
- The server opens a store connection per boundary, as every served read and
  write does; `server-first.md` has that as its own open item.
- The server rewrites the delivery file after a checkout move and the hook
  rereads it; changing that is another Task.

Settled by the design (concept review, November 10):

- A custom `LF_HOME` (a World, a source experiment) that launches Task work
  gets a launchd or systemd job for that home. Task work requires its
  Machine's server, so this is the decision's consequence, not a leak.
- A skill run in a Task's checkout carries the Task (`work_source`
  checkout) and so needs the server. It touches the work model; "anything that
  doesnt touch the work model" is the Task-less run, which stays server-free.
- Pi's `--moment pi-event` (`harness/pi.rs::native_event`) is a receipt
  observer like `__codex-observe`, not a turn-boundary hook: it moves with
  "Observers become server registrations" (`server-first.md`), not here.

Concept review with a person, November 11 (`scratch/concept-review.html`).
Items 2 and 3 are unanswered; the "Settled by the design" entries on the same
subjects were written headless and are open again.

1. Which launches need the server. Shown the two rows where the built check
   and `server-first.md`'s managed rule differ (`lf implement` typed in a Task
   checkout; a subagent `lf research` started by a Task's agent), Jack Heart:
   "I think these are both ok", then "no i mean 1b -- i think its ok that
   these dont get hooks". 1b on the page: launches through Task and Wave
   commands and Desktop need the server; a plain `lf <skill>` in a Task
   checkout runs without one and still counts toward the Task. Not built:
   `require_server_for_work` still asks `session_has_work`, and the plan's one
   managed function is unwritten. Today the start and compact hooks also print
   the context block (`context.rs::emit_block`), so a launch with no hooks
   needs its context in the launch prompt first; `server-first.md` has that as
   "Bare launches drop their hooks", after transcript telemetry.
   Told that, Jack Heart: "these should be lighter weight." and "if its a
   subagent the agent driving can steer". Implement next: one managed
   function (Task and Wave commands, Desktop); managed launches get hooks and
   the server check; bare launches get neither, with the context block in
   their launch prompt. A bare run's Session still carries its checkout's
   Task. Lost until transcript telemetry: a bare run's turn records and its
   context after a compaction.
2. A stale server passes the launch check (hello) and then answers no
   boundary (QA, beside installed 0.15.2). No server and a stale server leave
   the same conversation; only one refuses. Choices: as built, refuse both, or
   start both with one line at launch naming the fix.
3. Steer confirmation puts every steer through `uncertain` for a tool
   interval. Choices: keep it and show that state as `sent`, or have the hook
   send its deadline so a late server hands over nothing (no `.printed`, no
   `waiting`; a steer is lost only when a hook dies between read and print).
   "arrive twice" was said when the choice was lost-until-retried or twice.

Remaining:

- Demo with a real Claude turn in a Task Session. QA (`scratch/qa.html`) ran
  the same sequence with the branch CLI, a launchd server and a stand-in
  `claude` running the installed hooks: steer delivered and `emitted`, no hook
  holding the store, frozen and stopped server each one gap line, duplicate
  after a lost `.printed`. A real Claude login was not available.
- Gate: the suites under Checks marked gate, plus `harness::` (named in the
  Task's check, not yet run on this tree).
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
