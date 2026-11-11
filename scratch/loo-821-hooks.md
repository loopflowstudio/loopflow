# LOO-821: hooks call the Machine server

Built. Docs describe it: `rust/loopflow/src/server/README.md` ("Turn and tool
boundaries"), `rust/loopflow/src/harness/README.md`, `docs/troubleshooting.md`,
one-line updates in `docs/dev/{codebase,reference,server-first}.md`.

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

- `cargo test -p loopflow --lib -- server::boundary server::client server::schema server::wire lf::commands::context context_block` — pass (after compress)
- `cargo test -p loopflow --test integration session_steers::` — pass (server up, down, back)
- `transport_tests` hook tests (`native_drafts…`, `retained_hooks_survive_upgrade`,
  `continue_session…`, `native_work_survives…`) — pass; one run of 11 PTY tests
  in parallel failed two under load, all pass rerun
- Swift `ServerProtocolTests` (new `hookBoundarySharesTheRustServerContract`) — gate (`--swift`)
- clippy — pass (after compress)
- `context_launch_tests::`, `context_block_tests::` — gate
