# Connect to the local Loopflow server

```sh
lf server status   # readiness, socket, protocol revision, incarnation and connected clients as JSON
```

Installation prepares client access automatically. Desktop's first connection
requests readiness through the selected installation; subsequent connections and
terminal recovery never re-register a server. `lf server status` only observes.

The foreground `lf server run` belongs to the OS supervisor. Desktop
uses the service for all shared Work reads, writes and subscriptions, plus
existing terminal attachment. It links no embedded Loopflow library. Desktop's UI automation socket remains app-owned.

`installation/server.rs` owns registration and replacement. Its hidden
`install server` bridge is for local clients, not a separate setup workflow.

On macOS the signed selected app bundles `com.loopflow.server.plist` and
registers it with SMAppService. Explicit development Machines use separately
named launchd jobs. Linux uses a user systemd service. Closing Desktop does not
stop the service. Session owners retain their existing lifetime policy; this
service creates no PTYs, retention records or Graphs. Logout/reboot do not
preserve live processes. Disabled Background Items remain disabled. Upgrade
retires the previous
`com.loopflow.machine` registration before starting `com.loopflow.server`.
The old bundled plist remains only for SMAppService approval inspection/removal.
Development jobs and Linux units migrate their hashed labels the same way;
existing disabled/masked units require explicit enablement. The `machine.sock`
address and its lock remain stable so already-running relays can recover across
the rename. There is one listener, with no alias or alternate protocol path.
Work and credentials stay in place.

## Local protocol, revision 2

One connection per client over the same-user Unix socket. Every message is one
JSON object per line (`serde_json` never emits a raw newline). Messages are
capped at 32 MiB and connections at 64; a connection has five seconds to send
its first message and five seconds to take any message the server sends. The
socket and its directory are private; remote proxies must never forward this
endpoint. Remote admission is outside this revision.

`wire.rs` owns the Rust types; `swift/Loopflow/Services/ServerProtocol.swift`
mirrors them. Executable fixtures live in `tests/fixtures/dto/server/`.
`nc -U machine.sock` is a debugging tool: type `{"type":"status"}`.

```text
→ {"type":"hello","revision":2,"min_revision":2,"client":{"kind":"desktop","platform":"macos","name":"Loopflow","version":"0.15.1"}}
← {"type":"welcome","client_id":"…","revision":2,"server_revision":2,"min_revision":2,"instance":"…","build":"0.15.1","executable_sha256":"…","machine":{…},"capabilities":[…],"limits":{"max_message":33554432,"max_connections":64}}
→ {"type":"request","id":"a","operation":{"kind":"plan.get","directory":"/","wave":null,"task":null,"all":true}}
→ {"type":"subscribe","id":"b","operation":{"kind":"session.list","directory":"/repo","headless":false,"after":null,"limit":null}}
← {"type":"response","id":"a","trace_id":"…","result":{…}}
← {"type":"response","id":"b","trace_id":"…","result":null}
← {"type":"reading","id":"b","frame":{"answers":null,"unavailable":null,"body":{…}}}
→ {"type":"checkpoint","id":"b","checkpoint":3}
← {"type":"reading","id":"b","frame":{"answers":3,…}}
→ {"type":"cancel","id":"b"}
→ {"type":"request","id":"c","operation":{"kind":"task.run","directory":"/repo","issue":"LOO-1","from":null,"agent":null,"approve":false}}
← {"type":"response","id":"c","trace_id":"…","result":{"state":"running","issue":"LOO-1","run":"code from start","graph_process":"…","log":"…"}}
→ {"type":"subscribe","id":"d","operation":{"kind":"task.wait","issue":"LOO-1","until":"submitted"}}
← {"type":"reading","id":"d","frame":{"answers":null,"unavailable":null,"body":{"reached":true,"task":{…}}}}
← {"type":"ended","id":"d"}
```

The first message is `hello` with the revisions the client speaks. The server
chooses the newest revision both sides speak and answers `welcome` with it, a
server-assigned `client_id`, the server's own range, its incarnation (`instance` changes with every start), build,
executable SHA-256, the Machine reading (`null` while the registry is
unavailable or uninitialized), capabilities and limits. With no revision in
common it answers `error {code: incompatible}` naming the side to update in
its message and in `data.update` (`client` or `server`), then closes. This build speaks revision 2 only; the shape lets a later server speak
two. A revision-1 client's four-byte length prefix starts below `0x20`; the
server answers it a revision-1 framed `incompatible` error carrying `instance`
and closes. That sniff is the whole compatibility layer and goes with revision 1.

`status` before `hello` is the readiness probe: `status {revision, min_revision,
instance, executable_sha256, clients}`, then the server closes. `lf server status` and
installation use it so readiness never opens Work or speaks the full protocol;
`lf server status` also reports `listening` (a server accepted the connection),
`revision` and `clients`: each welcomed connection still open, oldest first, as
`{id, client, connected_at, requests}`, where `client` is the identity its
`hello` gave and `requests` counts its requests, subscriptions included. The
identity is for diagnostics and grants nothing. Local readiness compares the executable hash with the selected
installation, an installation check that never applies to a remote client.

After `welcome`, many requests share the connection. A `request` carries an `id`
(1–128 bytes, unique among the connection's unanswered requests) and an
`operation`; the server answers `response {id, trace_id, result}` or `error
{id, code, message, data}` in whatever order work finishes. `result` is the
operation's value: what its CLI spelling prints with `--json`, `{owner}` for
`session.owner`, and null once a subscription or attachment is confirmed.
`trace_id` names the request in `logs/lf.log`. `cancel {id}` forgets a request's
answer or ends a subscription; a write already dispatched still completes and
answers nothing. Closing the connection cancels every subscription and releases
every wait; admitted mutations finish. An `error` with a null `id` is about the
connection and is its last message: the server closes after `busy` at capacity
(nothing ran; retry shortly) and after `invalid` for a malformed line or a
repeated `hello`/`status`. `data` is structured detail or null. Clients branch
on `code`; only `outcome_unknown` means the request may have taken effect:

| `code` | Meaning | `data` |
| --- | --- | --- |
| `incompatible` | No revision in common | `{server: {revision, min_revision}, update}` |
| `invalid` | Malformed message or refused arguments, such as subscribing to a read with no watch | null |
| `not_found` | The named owner, subscription, repository checkout, view member or record does not exist | what was named: `{owner}`, `{id}`, `{repository}`, `{expression}` |
| `conflict` | The request contradicts current state: a still-unanswered request ID, or another viewer holding a Session's input | `{id}`, `{session}` |
| `unavailable` | A dependency is missing or failed: the Machine registry, a watch, a read | `{resource}` when one is named |
| `outcome_unknown` | The operation may have run; inspect state before retrying | null |
| `busy` | Capacity reached, or a subscription fell behind its checkpoints; retry shortly | `{max_connections}` at capacity |

A shared operation names its code by returning `engine::Error` inside its
error; anything else a read returns is `unavailable`.

Every operation names its `kind` as a dotted resource and verb, beside its
fields. `lf/commands/read.rs` (`Read`) and `lf/commands/write.rs` (`Write`) own
the shared kinds; `wire::ServerOperation` owns the server's own:

| Resource | Reads | Writes |
| --- | --- | --- |
| `task` | `task.get`, `task.list` (`closed: true`), `task.changes`, `task.diff`, `task.file`, `task.context`, `task.runs`, `task.wait` | `task.create`, `task.run`, `task.ship`, `task.hold`, `task.steer`, `task.comment`, `task.interrupt`, `task.update`, `task.move`, `task.claim`, `task.unassign`, `task.save`, `task.complete`, `task.reopen`, `task.abandon` |
| `session` | `session.list` (`limit` pages it), `session.owner`, `session.asks` | `session.rename`, `session.bind` (`preview` assigns nothing), `session.resume`, `session.new`, `session.answer` |
| `wave`, `chapter`, `plan` | `wave.list`, `wave.get`, `wave.graph`, `plan.get` | `wave.create`, `wave.sync`, `chapter.start` |
| `repository` | `repository.planning`, `repository.people`, `repository.tokens`, `repository.path` | |
| `graph` | `graph.list`, `graph.run` | `graph.customize` |
| `machine`, `process` | `machine.self`, `machine.list`, `machine.doctor`, `process.list` | |
| `usage`, `history` | `usage.get`, `history.list`, `history.retention` | |
| `work`, `view`, `config` | `work.resolve`, `view.list`, `view.get`, `config.settings` | `config.file` |
| `terminal` | | `terminal.attach` |
| `desktop` | `desktop.portfolio`, `desktop.layout` | `desktop.portfolio.set`, `desktop.layout.save`, `desktop.dashboard.query`, `.pin`, `.unpin`, `.rename`, `.hide`, `.show`, `.slots`, `.move` |

`desktop.*` is Desktop's own arrangement state: which repositories its windows
show, saved pane layouts and Dashboards. It is served for Desktop and is not
part of what a phone or bot client is offered.

- `config.file {request}` uses `config::api::Request` for catalogue, declaration,
  explanation and theme access. CLI and Desktop share the same Rust validation
  and file writer. It returns the value `config::api::run` returns and never
  replays a failed or uncertain save. Setting values are omitted from request telemetry.
- A read uses the existing `Read` shape and answers with the JSON the command
  prints, serialized once. A command that prints bare text
  (`config repository --path`) answers a JSON string. Every shared read is
  supported. Repository IDs resolve on the destination with `repository.path`;
  paths in local reads are destination paths. `view.get` returns the existing
  WorkViewPlan, including the recorded primary Session; roadmap membership alone
  cannot select one. A missing primary stays absent. Resolve its owner with
  `session.owner` before attachment.
- `subscribe {id, operation}` takes a read with a watch: `view.list`,
  `session.list` without `limit`, `session.asks`, `task.get`, `task.wait`,
  `history.list`, `plan.get`, `repository.planning` and `process.list`. It answers
  `subscribed`, then `reading {id,frame}` messages wrapping
  the existing complete `answers/unavailable/body` watch frame. `checkpoint
  {id,checkpoint}` asks for a reading now; the next reading's `answers` carries
  that number, changed or not. A subscription's arguments are fixed: new
  arguments are a new subscription with a complete first reading. A watch that
  fails sends its `unavailable` reading, then `error {id, code: unavailable}`
  ends the subscription; the connection stays open. A read with no watch is
  `invalid`. Reconnect opens a fresh subscription
  and receives a complete reading; no replay cursor exists. `task.wait {issue,
  until}` (`submitted` or `terminal`) reads `{reached, task}` whenever the Task
  changes; after the reading whose `reached` is true the server sends `ended
  {id}` and nothing more for it. Requested once, it is that reading now, as
  `lf task wait --timeout 0s --json` prints it.
- The core loop is `engine::task`, which the CLI handlers call too.
  `task.run {directory, issue, from, agent, approve}` and `task.ship
  {directory, issue}` outlive the request, so the server starts their
  command (`lf [-b] [-a <agent>] task run …`, `lf --task <issue> ship`)
  detached under its own execution owner and answers what that process
  reports once it knows: `{state: running, graph_process, log}` when the
  Graph registers, `waiting {step}` or `ended` when nothing ran, `shipping
  {process, log}`, or the refusal it printed. The connection never waits on
  the run. `approve` is `-b`: every step headless to `end`, approving
  Auto-Ship; without it the run stops before the next interactive step, and
  one with none ahead reaches `end` as bare `lf task run` does. A null `agent`
  uses each step's configured harness. `lf task run <issue> --json` and `lf
  ship --json` print the same value. `task.hold` withdraws the approval
  (`{issue, stopped}`), `task.steer` and `task.comment {message}` save to the
  thread and return it, and `task.interrupt` returns its receipt.
- A write uses the shared `Write` shape. `Write::run` calls the shared Rust
  function its CLI handler calls and returns the typed value; the CLI prints
  that value with `--json` and renders it as text otherwise. Nothing parses
  standard output. `task.create {directory,id,title}` adopts the selected
  conversation and returns the Task without launching work. A write whose Work
  lives on another Machine (`task.run`, `task.ship`, `task.hold`,
  `task.claim`, `task.unassign`, `session.resume`) runs its `lf … --json`
  spelling there over SSH and returns what that prints, which is the
  destination's value; it never falls back to local execution. Requests create no Process rows; diagnostics the shared
  functions print go to the request's log. A refusal the shared code names
  (an empty name is `invalid`, a stale Dashboard revision `conflict`, a missing
  record `not_found`) answers that code; any other failure is
  `outcome_unknown`, since the write may have taken partial effect. A lost
  response means outcome unknown; inspect state before retrying. IDs correlate
  diagnostics and are **not** durable deduplication receipts. No automatic
  resubmission.
- `session.asks {}` is what agents on this Machine wait on a person for, a
  complete list every reading, longest waiting first: `[{session, name, task,
  agent, since, question, kind, source}]`. A Session asks when Rust's
  Available judgment says it waits (`source: report` from the terminal's
  validated Program Status, `provider` from the provider's stream); its
  `question` is a blocked report's message, else the provider's last turn,
  else null, never a guess. A Task standing at an interactive step with no run
  carrying it asks too (`source: step`, `question` the step's name, `session`
  its primary conversation or null). `since` is Unix seconds, null when nothing
  recorded it. `lf session asks --json` prints the same list; Desktop's
  Available state for a Session is the same reading.
- `session.answer {session, text, submit}` types `text` into the Session's
  terminal through its owner, then Enter with `submit` (`\r` on a PTY, `\n`
  on pipes), and answers `{session, bytes, submitted}`. It attaches as a
  viewer asking for input control without replacing anyone, confirms each
  write with the owner before the next, and leaves. Another viewer holding
  input is `conflict` and nothing is sent; an unknown or ended Session or an
  owner that no longer answers is `not_found`; a running provider with no
  Loopflow owner is `unavailable`. A failure after the claim is
  `outcome_unknown`; nothing is resent. `text` is omitted from request
  telemetry. `lf session answer <session> <text> [--submit] --json` prints
  the same value.
- `session.owner {session}` resolves a Session ID to `{owner}`.
  A null owner means no recorded transport; resolution never starts a provider.
  An owner can finish between lookup and attachment, which then fails explicitly.
- `terminal.attach {owner}` takes the exact existing owner Process ID on a connection of
  its own: hello, one attach request, its response, then the unchanged binary owner
  protocol in `transport/README.md` and `transport/wire.rs`. An owner whose
  endpoint refuses is `not_found {owner}`: it ended. Each terminal has
  its own socket, so readings never wait behind terminal bytes and a control
  failure never closes a bridged terminal. The owner alone grants input/geometry,
  handles bounded output continuation and revokes stale control. No automatic
  owner replacement or raw-input replay. Desktop automatically reconnects the
  exact owner with bounded backoff, keeping connection status in native chrome
  outside terminal bytes. Recovery uses Resume for byte streams or a fresh
  terminal screen and requests free claims without replacement. Newer viewers
  retain input and geometry. Unavailable keeps the last screen marked stale;
  Retry reconnects without preparing or resuming a provider. A connection
  failure alone never means the Session ended.

Desktop holds one connection per server location for the app's lifetime,
opened on first use and reopened on the next request after a loss with bounded
backoff (100 ms doubling to 3.2 s). Its watches share that connection; losing it
fails every pending request as outcome unknown and every watch, which the
window reopens for a complete reading.

Every client gives the server ten seconds to answer `status`, `hello` or
`attach`; a server that accepts and stalls fails the open with that diagnosis
and the caller's backoff retries. A welcomed session's waits and the bridged
stream are unbounded unless the caller sets its own limit.

Request diagnostics correlate the response trace with its target, duration,
result, service incarnation and request ID. The existing log formatter adds the
real server PID and executable identity. Request spans and nested effect events
are enabled in normal `logs/lf.log` output; input bodies are omitted. Panic
unwinding logs an unknown outcome and restores the worker's scope.

`journal::request` isolates caller attribution and propagates it into nested
blocking operations. It never changes process cwd, environment or standard
streams. CLI Process records and provider usage stay with existing owners;
arrangement, observations and reconnects add no Process or usage records.
A connection error alone proves no command failure.
Service startup, command outcomes and failures use the existing `logs/lf.log`
diagnostics. Individual reads and subscription frames are not logged by default.
