# LOO-835: OpenCode and Pi ask the server from inside the provider

Intended behavior lives in the docs (maintained in place):
`rust/loopflow/src/server/README.md` "OpenCode and Pi ask from inside the
provider", `rust/loopflow/src/harness/README.md`, `docs/troubleshooting.md`,
`docs/dev/server-first.md` Pieces row, `docs/dev/reference-processes.md`
(Pi bash guard).

Built:

- `harness/server_connection.js`: one Node client both plugins embed. Each
  boundary is one `machine.sock` connection (connect, `hello` kind `plugin`,
  one `session.boundary`, close), serialized in report order, 2 s deadline over
  the whole exchange; gap and printed batches in plugin memory. Spec carries
  `server` (socket, revision) from `harness/context.rs::server_spec`.
- Pi: every reported event is `moment: "event"` (native) or `tool` (RPC-driven,
  steers only, during a turn). 1 s poll and `--moment pi-event` deleted.
  Start/compact go to the server; `lf __context-block` renders the block with
  `LF_PROVIDER_CONTEXT` removed, so it asks no server.
- Pi bash guard: `context.rs::pi_bash_guard` builds a `/bin/sh -c` script from
  `os_process::WATCHDOG` and puts it in the spec (`bashGuard`). It checks with
  `ps -o pgid=,ppid=` that the tool shell leads a detached group whose leader's
  parent is Pi, opens the FIFO read-write then read-only (never waits), and
  forks the watchdog with output to `/dev/null`. A failed check starts nothing.
- OpenCode: block rendered at first request and after compaction; `tool`
  before every other request; `stop` at turn end.
- Server: `BoundaryMoment::Event` → `harness::pi::observe_native`, steers only
  while `turn` is set and the event is not `agent_settled`.
- Deleted: `ContextMoment::PiEvent`, `ContextMoment::ToolGroup`,
  `--lifeline`/`--provider-pid`, `guard_tool_lifeline`, `watch_tool_lifeline`,
  `pi::native_event`, `SqliteStore::open_for_hook`, the kept socket and
  `close()` in the plugin client.

Why not one connection per turn (concept review, November 11): a plugin
connection held one of the server's 64 `MAX_CONNECTIONS` slots for a whole
turn; 64 Pi or OpenCode turns would push Desktop and the Claude and Codex
hooks into `busy`. The Task title still says "keep one server connection";
the PR body says why it doesn't.

Checks run here (all pass): `harness::context::tests::` (10: stand-in server,
counts lf processes; bash guard accept/refuse; probe runs a bash command and
starts no `lf`), `server::boundary::`, `harness::pi` (lib), `lf::tests::`,
`session_launch_contract pi_`, `documented_commands::`,
`cargo clippy --all-targets -D warnings`.

Left for demo/qa: a live managed Pi and OpenCode turn with `ps` during the turn
(Done line), steer arrival on a real provider, server stopped mid-turn, and a
Pi version whose local bash no longer detaches (the guard must refuse, or the
watchdog kills Pi at the first tool end).
Left for gate: affected suites once (`--swift` for the ServerProtocol comment).

Out of scope, seen here: native Pi keeps one long-lived
`lf __context-block --moment task-updates` child per conversation, the
same kind of process the Task counted (62 holding 8.3 GB). A server
subscription could replace it, but it would hold a slot for the conversation.
