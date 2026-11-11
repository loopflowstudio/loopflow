# LOO-821: hooks call the Machine server

Built: the `start`, `compact`, `tool` and `stop` moments of
`lf __context-block` make one `session.boundary` request and open no store.
Described in `server/README.md` ("Turn and tool boundaries"),
`harness/README.md`, `docs/troubleshooting.md` and `docs/dev/server-first.md`.

Implement next (decided November 10; exact words in `questions.md`):

- Task launches (`lf task run` and its Graph steps, `resume --task`, Task and
  Wave Sessions) start the Machine server before the agent starts and fail
  naming the server when it cannot start. Not built: see Open.

Looked at and left alone:

- `moment` and `callback.hook_event_name` both describe the boundary. The
  values are `--moment` arguments of hooks installed in live conversations
  (`retained_hooks_survive_upgrade`); renaming breaks those.
- The server opens a store connection per boundary, as every served read and
  write does; `server-first.md` has that as its own open item.
- The server rewrites the delivery file after a checkout move and the hook
  rereads it; changing that is another Task.

Open, with why it matters:

- Where a Task launch starts the server. `installation::server::ensure`
  registers a launchd job (`com.loopflow.server.<home hash>`, `KeepAlive`) or
  a systemd user unit for the selected home. Integration tests launch Task
  work with the real `lf` under hundreds of temporary homes
  (`transport_tests`, `local_planning`, `task_graph_launch_tests`, …) with no
  server; only three `transport_tests` fixtures register a job, and they boot
  it out. Wired where every launch reserves its hook capability
  (`harness/agent_process.rs::steer_environment`; the Session's Task or Wave
  is readable there), each test would leave a supervised server per temporary
  home, and CI Linux has no `systemctl --user`. Ways through: every
  Task-launch fixture starts and stops `lf server run`
  (`session_steers.rs::MachineServer`); or the launch only checks that a
  server answers and names `lf install server`; a tests-only switch is ruled
  out by the style guide. On a person's Machine the call is
  `readiness_command` (as `client::bridge`), not `ensure`: a branch build
  calling `ensure` restarts the installed server under live Sessions.

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
- `stop`: planning refresh and pending move run on a server blocking worker,
  spawned before the answer; the move waits 2 s for the turn to end.
- PostToolUse / UserPromptSubmit hook timeouts 30 s → 10 s.

Deleted: the hook's store opens (`hook_store`, `acquire_planning`,
`deliver_steers`), `ContextMoment::Move`, `apply_session_move`,
`start_pending_move`, `wait_for_hook_exit`, the hook path's Work binding in
`bin/lf.rs`, and `linear_on_demand_native_hooks_share_freshness_across_processes`.
Turn-end planning refresh now has no Linear-fixture test;
`server::boundary::after_stop` calls `sync_repository` directly.

Checks:

- `cargo test -p loopflow --lib -- server::boundary server::client server::schema server::wire lf::commands::context context_block` — 35 pass with steer confirmation; `store::sqlite::steers` — 17 pass
- `cargo test -p loopflow --test integration session_steers::` — 4 pass (server up, down, back; printed steer `uncertain` then `emitted` after the next hook; new `a_skill_with_no_task_runs_without_the_machine_server`). One earlier run failed two with 10 s provider-start timeouts at load average 416; same tree passed at 115.
- `cargo test -p loopflow --test integration dto_fixtures` — 50 pass
- `transport_tests` hook tests (`native_drafts…`, `retained_hooks_survive_upgrade`,
  `continue_session…`, `native_work_survives…`) — gate; last passed before the sync and before steer confirmation
- Swift `ServerProtocolTests` (new `hookBoundarySharesTheRustServerContract`) — gate (`--swift`)
- clippy — pass (with steer confirmation)
- `context_launch_tests::`, `context_block_tests::`, `harness::` — gate
