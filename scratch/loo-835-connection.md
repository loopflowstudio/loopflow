# LOO-835: OpenCode and Pi keep one server connection

Intended behavior lives in the docs (maintained in place):
`rust/loopflow/src/server/README.md` "OpenCode and Pi keep one connection",
`rust/loopflow/src/harness/README.md`, `docs/troubleshooting.md`,
`docs/dev/server-first.md` Pieces row.

Built (uncommitted on this branch until the next checkpoint):

- `harness/server_connection.js`: one Node client both plugins embed
  (`hello` kind `plugin`, one `session.boundary` per boundary, serialized,
  2 s deadline covering reconnect, gap and printed batches in memory,
  `close()` after the turn's `stop`). Spec carries `server` (socket, revision)
  from `harness/context.rs::server_spec`.
- Pi: every reported event is `moment: "event"` (native) or `tool` (RPC-driven,
  steers only, during a turn). 1 s poll and `--moment pi-event` deleted.
  Start/compact go over the connection; `lf __context-block` renders the block
  with `LF_PROVIDER_CONTEXT` removed, so it asks no server.
- OpenCode: block rendered at first request and after compaction (the old
  `compacted` never reset); `tool` on the connection for every other request;
  `stop` on the connection.
- Server: `BoundaryMoment::Event` → `harness::pi::observe_native`, steers only
  while `turn` is set and the event is not `agent_settled`.
- Deleted: `ContextMoment::PiEvent`, `pi::native_event`,
  `SqliteStore::open_for_hook` (no hook opens the store now).

Checks run here (all pass): `harness::context::tests::` (stand-in server,
counts lf processes), `server::boundary::`, `server::schema::tests::`
(schema fixture regenerated), `harness::pi` (lib), `session_launch_contract pi_`,
`documented_commands::`, `cargo clippy --all-targets -D warnings`.

Compress: Pi's `readContext` confirms its own steer batch and returns the text;
nothing else cut (`harness::context::tests::` 8/8 pass).

Left for demo/qa: a live managed Pi and OpenCode turn with `ps` during the turn
(Done line), steer arrival on a real provider, server stopped mid-turn.
Left for gate: affected suites once (`--swift` for the ServerProtocol comment).
