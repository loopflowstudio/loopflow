# Server protocol revision 2: one connection, negotiated revision, typed results

LOO-593 · design pass · November 5, 2026 (Pacific)

Jack Heart set the direction on November 5: use Rex and other relevant setups to
keep improving the server's architecture and protocols for performance,
reliability and elegance, with extra focus on mobile, while considering Desktop
and the "agent shed" (a headless host running agents, reached from a laptop or
phone). This design proposes revision 2 of the client protocol and names the
Tasks that deliver it. It changes no shipped code and approves no shipping.
Evidence: this branch at `22c297e120`, LOO-718's PR #1812 at `983b271695`, the
installed Rex server's API revision 8 specification, the saved
[measurements](server-performance/results.md) and the
[Rex comparison](rex-server-comparison.md).

## What the current protocol costs each client

Revision 1 ([README](../rust/loopflow/src/server/README.md)) is one operation
per connection: connect, send one framed request, read one framed response,
close. Subscriptions keep their connection open; nothing else does.

| Client | Where revision 1 hurts | Evidence |
| --- | --- | --- |
| Phone | Every read or write opens a new WebSocket: TLS handshake, HTTP upgrade and admission lookup, over Tailscale, per operation. The client comment says so: "One application operation per WebSocket." | `MachineConnection.swift` in #1812; `remote.rs` admits per connection and polls revocation per connection |
| Phone | Our four-byte frames are tunneled as a byte stream inside WebSocket binary messages, split into 64 KiB chunks. The phone reassembles frames the carrier could have delimited. Two framings, one meaning. | `remote.rs` `bridge`, `MachineConnection.receiveResponse` |
| Phone and shed | Exact revision match. A phone and its host, or a laptop Desktop and a shed, update on different days. The only answer today is `incompatible`. | `service.rs` `request.version != VERSION` |
| Desktop | Each shared read pays a connect and a detached thread; the result is pretty-printed JSON escaped inside a JSON string, parsed twice (server string, client value). | `wire.rs` `Reply::Read { output: String }`, `read.rs` `pretty()`; `ServerClient.read` returns `String` |
| Desktop | Command replies are stdout, stderr and an exit code. Callers re-decode stdout and infer failure from `code`. | `Reply::Command`, `ServerClient.commandOutput` |
| All | No way to ask the connected server what it speaks. Rust and Swift mirrors plus fixtures are the contract; a third client (phone, shed tooling) copies by hand. | `tests/fixtures/dto/server/`, `ServerProtocol.swift` |

The saved baseline: roadmap read median 26.8 ms over the server versus 21.1 ms
embedded; rename 10.8 ms direct. Locally the per-connection cost is a few
milliseconds. Remotely it is a round trip or three per operation. The numbers
argue for fixing the connection model for remote clients first and treating
local latency as a measurement, not a target.

## What Rex settles, and what it doesn't

Rex (installed build, API revision 8) opens one persistent control WebSocket per
client: a `hello` naming `api_revision` and `api_min_revision`, a `welcome`
naming the chosen revision, limits and a reconnect token, then concurrent
`request`/`response` pairs by string id and unsolicited `event` messages with a
`seq`. Control messages are capped at 1 MiB; a client more than 8 MiB of events
behind is disconnected. Terminal bytes travel on a separate data connection
admitted by a one-use ticket. Responses to in-flight requests are lost on
disconnect and may have executed; the client re-reads state. Events missed while
disconnected are lost; the client re-reads and applies later events.

What transfers to Loopflow: the connection model, the revision range handshake,
structured results and errors, and the separate terminal channel (we already
have it: `attach` becomes the owner protocol). What doesn't: Rex's event
reducers and `seq` ordering. Our watches already send complete readings, which
let a client replace a projection without reducers; nothing measured here says
that model is too slow. Rex's eight translation layers are a consequence of
eight revisions, not a design to copy; a small supported window is the point.

## Proposed revision 2

### Connection

A client opens one connection and keeps it. The first message is `hello`; the
server's first message is `welcome`. After that the client sends `request`,
`checkpoint` and `cancel` messages; the server sends `response` and `reading`
messages. Many requests may be in flight; each `id` is unique among the
client's unanswered requests. A subscription is a `request` whose `response`
is followed by `reading` messages carrying the same `id` until the client sends
`cancel` or the connection ends. Closing the connection cancels every
subscription and releases every waiting client; an admitted mutation still
completes, as today.

`attach` stays a dedicated connection: hello, one attach request, `bridged`,
then the unchanged binary owner protocol. Mixing terminal bytes into the
control connection would make every reading wait behind terminal output, and
it would tie the person's byte path to the control path's health. Jack's
November 5 direction after the upgrade incident applies here: lf is a session
manager and context adder on top of a harness, never a middleman that can break
the person's connection to their agent. The bridge therefore stays a plain byte
copy with no store read in its path, the control connection is the only place
the server consults Work, and a control failure, a `busy` reply or a bounded
queue overflow never closes a bridged terminal. Server replacement still drops
bridges; the owner and its Session survive, and Desktop reconnects.

```text
client ── hello{revision 2, min 2, client} ──▶ server
client ◀── welcome{revision 2, server 2, min 2, instance, build, limits, capabilities} ── server
client ── request{id a, read roadmap} ──▶
client ── request{id b, subscribe desktop_sessions} ──▶
client ◀── response{id a, value}
client ◀── response{id b, subscribed}
client ◀── reading{id b, frame}
client ── checkpoint{id b, checkpoint 3} ──▶
client ◀── reading{id b, …}
client ── cancel{id b} ──▶
```

### Carrier framing

The protocol is a sequence of JSON messages. Each carrier delimits them its own
way and the server code is written once against a message stream:

- Unix socket: one JSON message per line, as ACP, MCP and Codex app-server
  frame their stdio transports. `serde_json` never emits a raw newline inside a
  message, so the delimiter is safe, and `tokio_util::codec::LinesCodec` with a
  maximum length (already a dependency) replaces the hand-written
  `receive`/`respond` pair. `nc -U machine.sock` becomes a debugging tool. The
  address, lock and permissions do not change.
- WebSocket (LOO-718's remote carrier): one text message per protocol message,
  as Codex app-server's `ws://` listener does. The 64 KiB byte-chunking and the
  phone's frame reassembly are deleted.

A revision-1 client still running at upgrade sends a four-byte length whose
first byte is zero; a revision-2 message starts with `{`. The server sniffs the
first byte and answers an old client with a revision-1 `incompatible` error in
revision-1 framing, naming the fix. That is the whole compatibility layer: it
detects skew and names the fix, and it is deleted when revision 1 is gone.

Message limit stays 32 MiB locally so a Task save above 64 KiB keeps working.
The remote carrier sets the same cap on its WebSocket; a phone that cannot
show a 32 MiB reading is a product question for LOO-718, not a protocol one.

### Compatibility

```json
{"type":"hello","revision":2,"min_revision":2,
 "client":{"kind":"desktop","platform":"macos","name":"Loopflow","version":"0.15.1"}}
{"type":"welcome","revision":2,"server_revision":2,"min_revision":2,
 "instance":"…","build":"0.15.1","executable_sha256":"…","machine":{…},
 "capabilities":[…],"limits":{"max_message":33554432,"max_connections":64}}
```

The server chooses the newest revision both sides speak. With none in common
it answers `error {code: incompatible, message}` naming which side to update,
then closes. The supported window is a product decision per release, not a
translation framework; revision 2 ships supporting only revision 2, and the
code is shaped so a later server can speak 2 and 3.

Local readiness keeps comparing the server's executable hash with the selected
installation. That is an installation check (same build, same Machine), not a
protocol check, and it never applies to a phone or a shed.

`welcome` replaces `bootstrap`. `status` stays as a pre-hello probe for
`lf server status`, so readiness never needs to speak the full protocol.

### Results

- `read` answers `response {id, value}` where `value` is the JSON the shared
  reader produced, not a string containing it. `read.rs` already has the typed
  value in hand before `pretty()`; the server serializes once and the client
  decodes once.
- `reading {id, frame}` carries the subscription's request id, which alone
  names the subscription. `checkpoint {id, checkpoint}` is typed: the next
  reading's `answers` carries the number. A subscription's arguments are fixed;
  changing them is a new subscription.
- `command` answers `response {id, trace_id, code, value, output, stderr}`.
  When the write's CLI spelling prints JSON (its `argv` carries `--json`), the
  server parses stdout into `value`; otherwise `value` is null and `output`
  holds the text. Diagnostics stay beside the value. No second mutation
  implementation: the shared dispatcher still produces the text.
- Errors stay `{code, message}` with the existing `ErrorCode`. Rex adds an
  HTTP-style status; we have no HTTP consumer and add none.

### Reconnect, cancellation and liveness

No reconnect token and no replay. A client that loses its connection opens a
new one, re-sends hello and re-subscribes; each subscription starts with a
fresh complete reading. In-flight commands are outcome unknown, as today. This
is Rex's documented behavior for control events and responses too, so a
reconnect token would only buy client identity for display.

`cancel {id}` releases the client's wait and ends a subscription. A dispatched
mutation still completes. Request handlers already run in request-scoped
workers; cancellation drops the response path only.

Liveness: WebSocket carries ping/pong; the Unix carrier needs none because the
peer is the same kernel. The server drops a connection that sends no hello
within five seconds, and bounds the per-connection outgoing queue; a client
that cannot keep up with its readings is disconnected and recovers by
reconnecting, which already yields complete state.

Capacity: today a server at its connection limit accepts the socket and drops
it without a reply, so the client sees an EOF and must call the outcome unknown.
Revision 2 answers `error {code: busy}` and closes, so the client can back off
and retry a request it knows never ran. With one connection per client, 64
connections is many clients, not many operations.

### Discovery

`lf server describe --json` prints the JSON Schema of hello, welcome, request,
response, `Read` and `Write`, derived with `schemars` (already a dependency; the
served enums gain `JsonSchema`). The checked-in fixtures are validated against
that schema in Rust, and the Swift mirror test validates the same fixtures, so
drift fails a test before it fails a phone. A served `describe` operation is
not needed: a client that can connect has `lf` on the host to ask.

## The three clients

**Desktop.** One connection for the app's lifetime, opened on first use,
reopened with bounded backoff. Reads and writes become messages on it; the
detached thread per request goes away. Watches stop owning a socket each.
`ServerLocation` and readiness are unchanged.

**Phone (LOO-718).** One connection per foreground session, with the existing
device grant on the HTTP upgrade. Message boundaries are frames. The 20-second
ping and revocation poll stay per connection, now one instead of one per
operation. Background suspension closes the connection; foreground reopens
and re-subscribes. LOO-718 adopts this shape in its handoff section rather than
finishing on revision 1.

**Agent shed.** A Mac mini or Linux box runs agents without a Desktop. Today a
laptop Desktop reaches its Work with `lf --machine <m>` over SSH, one process
per read. The shed is the case where revision skew is normal and where a
persistent authenticated connection replaces per-command SSH for Work reads,
subscriptions and terminal attach. That is the same carrier and admission the
phone uses; execution launches, installation and file sync keep SSH. This
design makes revision 2 the contract that carrier speaks; wiring Desktop to use
it for a Machine is its own Task below, dependent on LOO-718's admission.

## What this deletes

- Hand-written length framing in `service.rs` and `client.rs`; the phone's
  byte reassembly and 64 KiB chunking in `MachineConnection.swift` and
  `remote.rs` `bridge`.
- `Reply::Read { output: String }` and every client-side second decode.
- `Operation::Bootstrap` (folded into welcome).
- A thread per Desktop request.
- The one-operation-per-connection rule and its per-operation admission on the
  remote carrier.

## Risks and how the design meets them

- **Head-of-line blocking.** A 32 MiB reading on the shared connection delays
  other responses. The outgoing queue is per connection and bounded; large
  readings are rare (Task files, history). Acceptable now; if measured, the
  answer is a second control connection for bulk reads, not events.
- **Slow consumers.** Bounded queue, disconnect on overflow, complete state on
  reconnect. Rex does the same at 8 MiB.
- **Lost replies.** Unchanged: outcome unknown, inspect before retrying. Request
  ids correlate diagnostics and do not deduplicate.
- **Two protocol versions in flight.** Desktop and the local server update
  together through installation, so revision 2 can ship without a translation
  layer. The remote carrier starts on revision 2.

## Demo

Desktop runs a session on one server connection: `lf ps` shows one connection
from the app where there were dozens; `lf server status --json` reports
`revision: 2`. The Swift performance harness reruns the saved workload and the
results file gains a revision-2 column for roadmap read, rename, first frame and
checkpoint. A remote-style run counts handshakes: thirty operations, one
handshake. `lf server describe --json` prints the schema and the fixture tests
validate against it.

## Tasks

1. **Revision 2 core** (stacked on LOO-593): connection model, hello/welcome,
   codec-based framing, typed read and command results, Desktop `ServerClient`
   on one connection, fixtures, README and `docs/dev/server.md`. Measured
   against the saved baseline.
2. **Discovery** (stacked on 1): `lf server describe --json`, schema-validated
   fixtures in Rust and Swift.
3. **Desktop reaches a Machine over the client carrier** (after LOO-718 lands
   admission): Work reads, subscriptions and attach to a shed over the
   authenticated WebSocket instead of per-command SSH.
4. **LOO-718 adopts revision 2**: recorded in its handoff; no separate Task.

## Other systems consulted

A read-only survey on November 5 of Zed's Agent Client Protocol, OpenAI's Codex
app-server, the Model Context Protocol (2025-06-18), Happy, tmux control mode,
mosh, Zed and VS Code remote servers, and JSON-RPC/Connect conventions. Sources:
[ACP initialization](https://agentclientprotocol.com/protocol/initialization),
[ACP transports](https://agentclientprotocol.com/protocol/transports),
[Codex app-server](https://learn.chatgpt.com/docs/app-server.md),
[MCP lifecycle](https://modelcontextprotocol.io/specification/2025-06-18/basic/lifecycle),
[MCP transports](https://modelcontextprotocol.io/specification/2025-06-18/basic/transports),
[Happy](https://happy.engineering/how-it-works/),
[tmux control mode](https://github.com/tmux/tmux/wiki/Control-Mode),
[mosh paper](https://mosh.org/mosh-paper.pdf),
[Zed remote development](https://zed.dev/docs/remote-development),
[VS Code reconnection PR](https://github.com/microsoft/vscode/pull/317774),
[JSON-RPC 2.0](https://www.jsonrpc.org/specification),
[Connect protocol](https://connectrpc.com/docs/protocol/).

| Pattern | Who | What revision 2 takes from it |
| --- | --- | --- |
| One connection, both directions, newline-delimited JSON | ACP, MCP, Codex (stdio); Codex `ws://` is one message per text frame and offers `unix://` for WebSocket over a Unix socket | Newline framing locally; one message per WebSocket frame remotely |
| Version handling splits: negotiate with a counter-offer (ACP, MCP, Happy) or ship the exact server build with the client (Zed, VS Code, Codex schema-per-binary) | both camps | Both, each where it fits: installation keeps Desktop and the local server on one build (the executable-hash check); hello/welcome negotiates for a phone or a shed |
| Capabilities gate optional features; additive capabilities are non-breaking | ACP, MCP, Codex | `welcome.capabilities` stays; a new operation is a capability before it is a revision |
| Disconnect is never cancellation; cancel is explicit | MCP, Codex `turn/interrupt`, ACP `session/cancel` | `cancel {id}`; a dropped connection releases waits but completes admitted mutations |
| A final snapshot is authoritative over streamed deltas | Codex `item/completed`, ACP config updates, mosh target state, tmux `capture-pane` after `%pause` | Keep complete readings; no event reducers |
| Overload is a retryable error with backoff guidance | Codex `-32001`, tmux `%pause`/`%continue` | `error {code: busy}` instead of a silent drop at capacity |
| Schema published out of band, not over the wire | ACP release schema, MCP TypeScript source, Codex `generate-json-schema` | `lf server describe --json`, not a served `describe` operation |
| Resume by fresh snapshot or by cursor, chosen per client | MCP `Last-Event-ID`, ACP `session/load` vs `session/resume`, Codex `thread/resume` | Fresh snapshot; our readings are already complete, and the terminal channel owns its own byte cursor |
| Pending prompts are re-delivered to a reconnecting client | Codex, Happy | Already true: prompts live in Session owners and reappear in the next complete reading |
| The host outlives the connection; clients re-attach | tmux, mosh, Zed daemon, Happy daemon, Codex threads | Already our boundary: execution owners are independent of the access server |

Two things the survey argues against. A reconnect token (Rex, VS Code) buys
client identity across connections; nothing in Desktop or the phone needs that
identity yet, and our readings make replay unnecessary. A relay (Happy) decouples
phone from host reachability; LOO-718 chose host Tailscale, and a hub remains
optional ingress in the accepted design.
