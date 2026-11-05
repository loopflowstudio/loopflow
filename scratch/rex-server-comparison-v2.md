# Loopflow revision 2 and Rex: server architecture and wire protocol

LOO-593 · regenerated against the revision-2 implementation · November 5, 2026 (Pacific)

Jack Heart asked for the earlier [Rex comparison](rex-server-comparison.md) to be
regenerated against protocol revision 2. Loopflow evidence: the revision-2
implementation in Task 4877de's worktree (`server/{wire,service,client}.rs`, its
README, `ServerProtocol.swift`, `ServerClient.swift`, fixtures under
`tests/fixtures/dto/server/`), uncommitted at the time of writing and about to be
folded into this branch. Rex evidence is unchanged: the installed Mac server,
API revision 8, its served specification and `api list --json`. Read-only; no
Rex state changed.

**Revision 2 closes the protocol gap the first comparison found.** Loopflow now
has the connection model, revision negotiation, structured results and
connection-level errors that Rex had and revision 1 lacked. The remaining
differences are deliberate: Loopflow keeps complete readings instead of event
reducers, publishes its schema out of band instead of over the wire, and has no
reconnect token because nothing needs client identity across connections. The
system boundary did not move: execution owners stay independent of the access
server, and the terminal keeps its own connection.

## What changed between revision 1 and revision 2

| Boundary | Revision 1 (PR #1813 as published) | Revision 2 (folded into #1813) |
| --- | --- | --- |
| Connection | One operation per connection; subscriptions kept theirs open | One connection per client for the app's lifetime; requests, subscriptions and cancels multiplexed by id |
| Framing | Four-byte big-endian length, hand-written on both sides | One JSON object per line through `LinesCodec`; `nc -U machine.sock` debugs it |
| Compatibility | Exact `version: 1` or `incompatible` | `hello {revision, min_revision, client}` → `welcome {revision, server_revision, min_revision, …}`; the server picks the newest common revision and names the side to update when none exists |
| Readiness | `status` request | `status` before hello, then close; `lf server status` reports `listening` and `revision` |
| Read result | Pretty JSON inside a string, parsed again by the client | `read {value}`: the JSON the command prints, serialized once |
| Command result | `trace_id, output, stderr, code` | `trace_id, code, value, output, stderr`; `value` is stdout parsed when the spelling prints JSON |
| Subscription | Its own connection; checkpoints as raw frames | `subscribed`, then `reading {id, generation, frame}`; `checkpoint {id, frame}`; `cancel {id}`; a failing watch ends with `error {id, unavailable}` and the connection stays open |
| Errors | `result: {kind: error}` per response | One `error {id, code, message}`; `id` null for connection-level errors |
| Capacity | Socket accepted and dropped silently | `error {busy}` before close: nothing ran, retry shortly |
| Old clients | n/a | A leading byte below `0x20` is a revision-1 length prefix; the server answers a revision-1 framed `incompatible` and closes. The one compatibility branch, deleted with revision 1 |
| Desktop client | A socket and a detached thread per request | One `ServerLink` per server location; reopened on the next request after loss with 100 ms doubling to 3.2 s; watches share it and reopen for a complete reading |

Deleted: `bootstrap` (folded into `welcome`), per-response `instance` (now in
`status` and `welcome`), the hand-written `receive`/`respond` framing, the
pinned-future workaround in the old subscription loop, and the thread per
Desktop request.

## The wire, side by side

| Boundary | Loopflow revision 2 | Rex API revision 8 |
| --- | --- | --- |
| Control carrier | Same-user Unix socket, one JSON object per line, one connection per client. Remote carrier (LOO-718) will carry one message per WebSocket text frame. | Persistent `/api/control` WebSocket, `rex.control.v1`, JSON text messages. Local endpoint is also a Unix socket. |
| Opening | `hello {revision, min_revision, client{kind, platform, name, version}}` within 5 s → `welcome {revision, server_revision, min_revision, instance, build, executable_sha256, machine, capabilities, limits}`. `status` before hello is the readiness probe. | `hello {api_revision, api_min_revision, client{kind, platform, name, version, label}, reconnect_token?}` within 5 s → `welcome {protocol, api_revision, api_server_revision, api_min_revision, limits, client{id, reconnect_token, resumed}, features}`. |
| Compatibility | Newest common revision; none → `error {incompatible}` naming the side to update, then close. This build speaks 2 only; the shape allows two. | Newest common revision; none → close 1008 "no common API revision". This build speaks 0–8 and translates older shapes. |
| Request and result | `request {id, operation{kind, …}}` → `response {id, result{kind, …}}` or `error {id, code, message}`; ids 1–128 bytes, unique among unanswered requests; answers arrive as work finishes. Typed `Read`/`Write` inputs; typed `value` results. | `request {id, method, payload}` → `response {id, seq, payload}` or `{…, error{code, message, status}}`; ids unique among pending; dependent calls wait for their predecessor. |
| Live updates | `subscribe` → `subscribed`, then complete `reading` frames per watch; `checkpoint` acknowledges; `cancel` ends. Reconnect re-subscribes for a fresh complete reading; no cursor. | Unsolicited `event {seq, …}`; a response's `seq` says which events to apply on top. Events missed while disconnected are lost; the client re-reads state. |
| Cancellation | `cancel {id}` forgets an answer or ends a subscription; a dispatched mutation completes and answers nothing. Closing cancels everything and releases waits; admitted mutations finish. | No cancel method for a request; closing the connection drops pending responses, which may still have executed. |
| Terminal channel | `attach {owner}` on a connection of its own: hello, one request, `bridged`, then the unchanged binary owner protocol. A control failure never closes a bridged terminal. | Expiring single-use ticket from the control connection, then `/api/data` WebSocket with a 17-byte binary HELLO; output only, input via control methods. |
| Recovery | No reconnect token. Desktop reopens with bounded backoff; pending requests fail as outcome unknown; watches reopen. Owner stream keeps its own reconnect token and byte position. | Control: reconnect token rotates per welcome, 60 s retention, `resumed` flag. Data: token plus consumed byte count, 60 s retention. |
| Errors | `incompatible, invalid, unsupported, unavailable, outcome_unknown, busy`; message for people. | String `code`, `message`, HTTP-style `status`; branch on code, fall back to status. |
| Bounds | 32 MiB messages, 64 connections, 5 s first message, 5 s per send, 64-message outgoing queue and 64-checkpoint inbox per connection; a consumer that falls behind is disconnected and reconnects for complete state. | 1 MiB control messages, 8 MiB event backlog before disconnect, 64 KiB data messages, 60 s reconnect retention. |
| Capacity | `error {busy}` before close. | Not documented as a distinct close reason; rate limits are per method. |
| Discovery | Rust/Swift mirrors and executable fixtures; `lf server describe --json` is the next Task. | Server publishes method schemas, examples and validation notes at `/llms.txt` and `api list`. |

Sources: revision-2 `server/README.md`, `wire.rs`, `service.rs` and
`ServerClient.swift` in the 4877de worktree;
[Rex method reference](https://www.superlogical.com/rex/docs/reference/api),
[Rex API discovery](https://www.superlogical.com/rex/docs/automate/server-api),
[Rex upgrades](https://www.superlogical.com/rex/docs/linux/upgrading) and the
installed server's `/llms.txt`.

## What is now the same

- One persistent connection per client, hello first, concurrent requests by
  string id, server messages in completion order.
- Revision negotiation with a min/max range and an explicit "update this side"
  outcome.
- Structured results and a single error shape with a machine-readable code.
- A five-second first-message deadline.
- Terminal bytes on their own connection, admitted through the control path.
- Lost replies are outcome unknown; neither side replays mutations.
- Disconnect loses in-flight answers and live updates; the client re-reads.

## What is deliberately different

| Choice | Loopflow | Rex | Why Loopflow chose this |
| --- | --- | --- | --- |
| Updates | Complete readings per watch | Events with `seq` and reducers | Clients replace a projection without reducer code; no measured workload shows readings are too slow. The terminal stream already owns a byte cursor. |
| Reconnect identity | None | Rotating reconnect token, `resumed` | Nothing in Desktop or the phone needs client identity across connections yet; readings make replay unnecessary. Add it when something does. |
| Schema | Out of band (`lf server describe`, fixtures) | Served over the wire | A client that can connect has `lf` on the host; ACP, MCP and Codex app-server publish the same way. |
| Message limit | 32 MiB | 1 MiB | A Task save above 64 KiB and large Task-file readings go through the control connection today. Head-of-line blocking is accepted until measured; a bulk connection is the fallback, not events. |
| Cancellation | Explicit `cancel {id}` | None | Lets Desktop release a wait or end one watch without dropping the shared connection. |
| Compatibility window | One revision at ship, two by shape | Nine revisions with translation | Installation keeps Desktop and the local server on one build; negotiation is for a phone or a shed. A translation framework is a cost paid per revision, not a design to pre-build. |
| Capacity | `busy` reply | Close without reply | The client knows nothing ran and can retry safely. |
| Framing locally | Newline-delimited JSON | WebSocket over Unix | No HTTP handshake or masking locally; the remote carrier maps one message per WebSocket frame and shares every type above the framing. |

## The three clients after revision 2

**Desktop.** One `ServerLink` per server location, opened on first use. Reads,
writes and watches are messages on it. Loss fails pending requests as outcome
unknown and reopens watches for a complete reading. The thread per request is
gone. Readiness and installation are unchanged.

**Phone (LOO-718).** Adopts the same message set with one message per WebSocket
frame, one connection per foreground session, admission on the HTTP upgrade.
Its byte-stream reassembly and 64 KiB chunking go away. Recorded in the
revision-2 plan's "Phone adopts" section for LOO-718 to take up.

**Agent shed.** The negotiation now exists for the case where a laptop Desktop
and a headless host are different builds. Wiring Desktop to reach a Machine
over the client carrier instead of per-command SSH remains Task 4c4b2d, after
LOO-718 lands admission.

## What to look at in review

- `service.rs`: the hello/status branch and the `0x20` sniff; the per-connection
  outgoing queue and what happens when it fills; `cancel` for a read in flight.
- `wire.rs` and the fixtures: no defaults anywhere; `welcome.machine` is the one
  `Option`.
- `ServerClient.swift`: `ServerLinks` backoff and how a lost link fails pending
  requests and watches.
- `server-performance/results.md`: the revision-2 column against the saved
  direct-write baseline, once the harness rerun lands.
- The handshake count: one connection for the thirty-one-iteration workload.

Comparison artifact only. LOO-593 remains at its demo step; nothing here
approves a merge or Auto-Ship.
