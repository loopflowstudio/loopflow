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
→ {"type":"request","id":"a","operation":{"kind":"read","query":{"read":"roadmap",…}}}
→ {"type":"request","id":"b","operation":{"kind":"subscribe","query":{…}}}
← {"type":"response","id":"a","result":{"kind":"read","value":{…}}}
← {"type":"response","id":"b","result":{"kind":"subscribed"}}
← {"type":"reading","id":"b","frame":{"answers":null,"unavailable":null,"body":{…}}}
→ {"type":"checkpoint","id":"b","checkpoint":3}
← {"type":"reading","id":"b","frame":{"answers":3,…}}
→ {"type":"cancel","id":"b"}
```

The first message is `hello` with the revisions the client speaks. The server
chooses the newest revision both sides speak and answers `welcome` with it, a
server-assigned `client_id`, the server's own range, its incarnation (`instance` changes with every start), build,
executable SHA-256, the Machine reading (`null` while the registry is
unavailable or uninitialized), capabilities and limits. With no revision in
common it answers `error {code: incompatible}` naming the side to update, then
closes. This build speaks revision 2 only; the shape lets a later server speak
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
`operation`; the server answers `response {id, result}` or `error {id, code,
message}` in whatever order work finishes. `cancel {id}` forgets a request's
answer or ends a subscription; a command already dispatched still completes and
answers nothing. Closing the connection cancels every subscription and releases
every wait; admitted mutations finish. An `error` with a null `id` is about the
connection and is its last message: the server closes after `busy` at capacity
(nothing ran; retry shortly) and after `invalid` for a malformed line or a
repeated `hello`/`status`. `ErrorCode` is `incompatible, invalid, unavailable,
outcome_unknown, busy`.

- `configuration {request}` uses `config::api::Request` for catalogue, declaration,
  explanation and theme access. CLI and Desktop share the same Rust validation
  and file writer. It returns the ordinary command receipt and never replays a
  failed or uncertain save. Setting values are omitted from request telemetry.
- `read {query}` uses the existing `Read` shape and answers `read {value}`: the
  JSON the command prints, serialized once. A command that prints bare text
  (`config repository --path`) answers a JSON string. Every shared read is
  supported. Repository IDs resolve on the destination with `repository_path`;
  paths in local reads are destination paths. `view_plan` returns the existing
  WorkViewPlan, including the recorded primary Session; roadmap membership alone
  cannot select one. A missing primary stays absent. Resolve its owner with
  `session_owner` before attachment.
- `subscribe {query}` supports every shared read with a watch. It answers
  `subscribed`, then `reading {id,frame}` messages wrapping
  the existing complete `answers/unavailable/body` watch frame. `checkpoint
  {id,checkpoint}` asks for a reading now; the next reading's `answers` carries
  that number, changed or not. A subscription's arguments are fixed: new
  arguments are a new subscription with a complete first reading. A watch that
  fails sends its `unavailable` reading, then `error {id, unavailable}` ends the
  subscription; the connection stays open. Reconnect opens a fresh subscription
  and receives a complete reading; no replay cursor exists.
- `command {change}` accepts the shared `Write` shape. The server calls the same
  Rust mutation dispatcher as the CLI, with the request's directory and streams.
  `command {trace_id,code,value,output,stderr}` returns its exit status, output
  and diagnostics; `value` is stdout parsed as JSON when the command's spelling
  carries `--json` and stdout parses, otherwise null. `task_from_session
  {directory,id,title}` adopts the selected conversation and returns the Task
  without launching work; CLI and server share its creation handler. Ordinary
  writes start no CLI child. Machine routing still uses SSH and never falls back
  to local execution. Requests create no Process rows. A lost response means
  outcome unknown; inspect state before retrying. IDs correlate diagnostics and
  are **not** durable deduplication receipts. No automatic resubmission.
- `session_owner {session}` resolves a Session ID to `session_owner {owner}`.
  A null owner means no recorded transport; resolution never starts a provider.
  An owner can finish between lookup and attachment, which then fails explicitly.
- `attach {owner}` takes the exact existing owner Process ID on a connection of
  its own: hello, one attach request, `bridged`, then the unchanged binary owner
  protocol in `transport/README.md` and `transport/wire.rs`. Each terminal has
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
