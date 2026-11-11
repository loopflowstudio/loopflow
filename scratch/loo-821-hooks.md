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

Open, with why it matters:

- A custom `LF_HOME` (a World, a source experiment) with no server gets a
  launchd or systemd job for that home the first time it launches Task work;
  that is what `lf install server` does there.

Remaining:

- Demo with a real Claude turn in a Task Session (server up: turn recorded,
  steer received, `ps` shows no `lf` on the store; server stopped: turn
  completes, one gap line). Not run. `session_steers::` covers the same
  sequence headless with a provider stand-in.
- Gate: the suites under Checks marked gate, plus `harness::` (named in the
  Task's check, not yet run on this tree).
- Pi's `--moment pi-event` (`harness/pi.rs::native_event`: turn, usage and
  completion receipts) still opens the store at each Pi turn, the last
  `open_for_hook` caller. The Task names Claude and Codex boundaries; whether
  Pi's receipts move to the server here or with the observer Task is open
  (`questions.md`). `limit` / `record-limit` also open the store, on a usage
  limit only, and are untouched.

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

Checks (November 10, on the launch check):

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
