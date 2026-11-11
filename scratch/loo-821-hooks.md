# LOO-821: hooks call the Machine server

Built: the `start`, `compact`, `tool` and `stop` moments of
`lf __context-block` make one `session.boundary` request and open no store.
Docs describe it: `rust/loopflow/src/server/README.md` ("Turn and tool
boundaries"), `rust/loopflow/src/harness/README.md`, `docs/troubleshooting.md`,
`docs/dev/server-first.md` (header and Pieces row), one-line updates in
`docs/dev/{codebase,reference}.md`.

Implement next (decided November 10; exact words in `questions.md`):

- Steer confirmation, replacing deliver-on-answer: the answer names its batch,
  the hook keeps the batch it printed beside the delivery file and sends it
  with its next request, the server marks that batch `emitted` and returns
  other unconfirmed batches older than the hook deadline to pending. A
  duplicate after a hook dies before writing its marker is accepted.
- Managed Sessions require the Machine server; no fallback to direct store
  writes. The gap line and `lf session steers` name the server as the missing
  piece.

Looked at and left alone:

- `moment` and `callback.hook_event_name` both describe the boundary, and
  `tool` also means a prompt. The values are the `--moment` arguments of hooks
  already installed in live conversations (`retained_hooks_survive_upgrade`);
  renaming them breaks those.
- The server opens a store connection per boundary. Every served read and
  write does today; "the server is the only process that opens the store" is
  its own open item in `server-first.md`.
- The server rewrites the delivery file after a checkout move and the hook
  rereads it. Awkward, but the alternative is the mover rewriting every
  delivery file, which is a different Task.

Open, with why it matters:

- A steer handed to a hook that had already given up is marked `emitted` and
  nobody sees it until someone runs `--retry`. Before this Task `emitted`
  meant the hook wrote it to the provider; now it means the server put it in
  an answer. The loss needs a server slower than two seconds, and the Machine
  this plan was measured on had load averages of 330 / 178 / 120 on November
  10, so "slow" is the case the plan exists for. Stopped and slow servers
  also behave differently for the same person: a steer saved while the server
  is down arrives at the next answered boundary; one claimed by a late answer
  does not. The repair is a confirmation: the answer names its batch, the
  hook keeps the batch it printed beside the delivery file and sends it with
  its next request, the server marks that batch `emitted` and puts any other
  unconfirmed batch back to pending. Two traps in it: parallel `PostToolUse`
  hooks arrive before the first has printed, so "unconfirmed" needs to mean
  older than the hook's deadline; and a hook that printed but died before
  writing its marker gets the steer twice. Built as deliver-on-answer; the
  choice is between a steer that can be lost until retried and one that can
  arrive twice.
- A Machine whose server never runs (Background Items disabled, `lf` without
  `lf install`) records no turns and delivers no steers, with one gap line in
  `logs/lf.log`. `lf session steers` then reads "capability reserved; no
  authenticated callback observed (hook may be unavailable or untrusted)",
  which does not name the server. Before this Task those Machines recorded
  and delivered. Whether lf is supported without its server is the question;
  `server-first.md` says bare runs need none, and bare runs keep these hooks
  until "Bare launches drop their hooks" lands.

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
- Server answers `{steers}` only. The hook renders the block from checkout
  files; checkout relocation moved to the server, which rewrites the delivery
  file, and the hook rereads it after an answer.
- One two-second deadline for connect, hello and answer (`client::hook_request`).
- Server failures (store, foreign conversation) answer `unavailable`; the hook's
  marker is the only gap memory. No per-Session gap state in the server.
- Steers are marked emitted when the server writes the answer (deliver-on-answer).
  A hook past its deadline loses that batch; `lf session steers --retry` resends.
- After a delivered batch the hook drops its seen marker, so the next tool
  boundary asks once more (replaces the `remaining` signal).
- `stop`: planning refresh and pending move run on a server blocking worker,
  spawned before the answer; the move waits 2 s for the turn to end.
- PostToolUse / UserPromptSubmit hook timeouts 30 s → 10 s; start/compact stay 30 s.

Deleted: the hook's store opens (`hook_store`, `acquire_planning`,
`deliver_steers`), `ContextMoment::Move`, `apply_session_move`,
`start_pending_move`, `wait_for_hook_exit`, the hook path's Work binding in
`bin/lf.rs`, and `linear_on_demand_native_hooks_share_freshness_across_processes`
(+ its child). Compress: the wire's `SessionBoundary` struct is what the server answers
(no server-side copy); `ContextDelivery::load` is the one decoder; the hook's
answer is `Option<SessionBoundaryAnswer>` (None = unanswered). The checked-in
schema now says `adapter` is nullable. Turn-end planning refresh now has no
Linear-fixture test;
`server::boundary::after_stop` calls `sync_repository` directly.

Checks:

- `cargo test -p loopflow --lib -- server::boundary server::client server::schema server::wire lf::commands::context context_block` — 34 pass after `reason` left `BoundaryGap`. `callbacks_select_exact_session…` failed once in that run: it set `LF_HOME` without `journal::test_env_lock` beside `tool_hook_starts_lf_only_after_a_newer_steer`; it takes the lock now.
- `cargo test -p loopflow --test integration session_steers::` — 3 pass after the LOO-819 sync (server up, down, back; one gap line, one `store-gap` event)
- `transport_tests` hook tests (`native_drafts…`, `retained_hooks_survive_upgrade`,
  `continue_session…`, `native_work_survives…`) — pass before the sync; one run of 11 PTY tests
  in parallel failed two under load, all pass rerun
- Swift `ServerProtocolTests` (new `hookBoundarySharesTheRustServerContract`) — gate (`--swift`)
- clippy — pass (after `reason` left `BoundaryGap`)
- `context_launch_tests::`, `context_block_tests::`, `harness::` — gate
