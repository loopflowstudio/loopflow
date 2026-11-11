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

Left: a Pi version whose local bash no longer detaches (the guard must refuse,
or the watchdog kills Pi at the first tool end). Only Pi 1.3.0 is installed;
the refuse path has its unit test only.

Out of scope, seen here: native Pi keeps one long-lived
`lf __context-block --moment task-updates` child per conversation, the
same kind of process the Task counted (62 holding 8.3 GB). A server
subscription could replace it, but it would hold a slot for the conversation.

Compress, November 11: no cut found past 33b7cb702. `git grep` finds no
`tool-group`, `--provider-pid`, `watch_tool_lifeline`, `PiEvent`,
`open_for_hook` or kept-socket code outside release notes; `lf context`:
scratch 7346/12000 tokens.

## Concept review, November 11 (headless, after compress)

`ps` on this Machine at the review: 96 live `__context-block` processes, all
`--moment task-updates`, 5.4 GB RSS. 60 under `codex` (4,556 MB, about 76 MB
each), 34 under `claude` (796 MB), 2 under `lf`. None under Pi or OpenCode,
and no per-step `tool`/`pi-event` process. The Claude and Codex ones come from
the async `task-updates` command in SessionStart and Stop
(`context_block.rs:296,301`); Pi's is `watchUpdates` in `context_pi.mjs`. So
the Task's measured problem (62 processes, 8.3 GB) matches the Task-update
watcher, not the per-step plugin processes this branch removes.

Done after the review: `server/README.md` "OpenCode and Pi ask from inside
the provider" now opens by naming the two `lf` processes left after launch
(one `__context-block` per compaction, Pi's task-updates watcher).
`harness/README.md` and `docs/dev/server-first.md` already agree.

PR body: does not claim the 62 / 8.3 GB figure as this Task's problem. Say
what the branch removes (per-boundary and per-second `lf` processes on Pi and
OpenCode) and that the measurement counted Task-update watchers, which LOO-826
deletes. Open questions: `questions.md` (LOO-835).

Concept review again, November 11 (headless, at 8cbd82d59): no change. The
only commit since the last review is the README opening it asked for; the two
scope questions in `questions.md` keep their reversible defaults.

QA, November 11 (`scratch/qa.html`): the Done line holds with real Pi 1.3.0
and OpenCode 2.2.1 against a scripted local model. Left for gate: affected
suites once (`--swift` for the ServerProtocol comment). Open from qa:
`questions.md` (frozen-server Pi slowdown, double comment delivery).

## Concept review with a person, November 11 (at demo)

Seen in this Task's own conversation: 25 `Loopflow worker progress` posts from
11:41 to publish, one per settled step, each starting a full turn answered
with one line; only "PR published" and "at demo" were actionable. `ps` at the
review: 83 `--moment task-updates` watchers, 5,690 MB.

Proposed, not decided: one Session inbox. Comments and settled run progress
are items drained by every `session.boundary` answer with steer confirmation;
wake is a per-item property (comment, run stopped or failed, interactive step,
PR published), and the waiter is a `/bin/sh` on a server-written file, not an
`lf`. Questions 1-4 in `scratch/concept-review.html`.

Jack Heart, November 11, quoting the three "Proposed" paragraphs above (one
Session inbox; wake as a per-item property for a person's comment, a run
stopping, an interactive step, a PR published; a `/bin/sh` waiter for
Claude): "this is ok with me". Not yet said: question 3 (ship #1889 and build
the inbox in LOO-826, or here) and question 4 (frozen-server Pi).

Answers to `concept-review.html`, Jack Heart, November 11: "1a 2a 3 dont care
4b". Asked again about 1 (1a, wake on every step, against the inbox paragraph
called ok), answered "Actionable + opt-in stream" (1c: actionable items wake
by default; a Session can ask to be woken on every step).

Built for 4b: while a gap is open, Pi skips every boundary except session
start, tool start and the turn end (`server_connection.js` `skippable`);
test `a_native_pi_turn_against_a_frozen_server_waits_only_where_a_steer_can_land`
(6 waits instead of 9).

The inbox (2a) and the wake rule (1c) go to LOO-826 (PR #1890), which already
moves Task updates into the `session.boundary` answer and wakes idle
conversations from the server.
