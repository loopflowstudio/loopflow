# Hold one server connection per client and negotiate its protocol revision

Task 4877de · implementation plan · November 5, 2026 · stacked on LOO-593 (PR #1813)
· reconciled November 5 after the handshake-bound compress

Jack Heart set the direction on November 5: improve the server's protocol for
performance, reliability and elegance with mobile, Desktop and headless agent
hosts in mind. The parent's accepted design is
`81cf2ee219:scratch/server-protocol-2.md` (retired from this checkout for
context budget); this Task delivers its "Revision 2 core" slice. Discovery (`lf server describe`), Desktop reaching a
Machine over the client carrier, and LOO-718's adoption are separate Tasks.
Parent notes: [loo-593-parent.md](loo-593-parent.md). Decisions and
assumptions: [questions.md](questions.md).

## Problem and outcome

Revision 1 was one operation per connection, hand-framed with a four-byte
length, exact-revision only. Desktop opened a socket and a detached thread per
read and decoded pretty JSON out of a JSON string; a phone would pay a TLS
handshake per operation; a skewed client got `incompatible` and nothing else;
at capacity the server dropped the socket silently.

A client now opens one connection, says `hello`, gets `welcome` with the
negotiated revision, and multiplexes requests, subscriptions and cancels by id
for the connection's lifetime. Reads answer JSON values. Commands answer a
parsed value beside their diagnostics. Messages are newline-delimited JSON on
the Unix socket. `attach` keeps its own connection and the unchanged binary
owner protocol.

## Status

**In PR #1813 (the parent), not this branch's diff.** Jack Heart folded this
Task's tree into LOO-593's branch at 09:04 on November 5 (`7883f6b402`, "Speak
server protocol revision 2"): wire types and fixtures, the per-connection server
session, the Rust client, the Swift mirror and one-connection `ServerClient`,
README and `docs/dev/server.md`, the revision-2 measurement column in
[results.md](server-performance/results.md) with one admitted connection for the
whole workload, both concept-review contract repairs (null-id errors close the
connection, with a unit test; `ErrorCode::Unsupported` deleted; `Display` for
`ErrorCode`) and the handshake bound with a stalled-server test on each side.
This checkout's base moved to that branch head (`81cf2ee219`).

**On this branch beyond the parent** (`git diff origin/jack/loo-593-… HEAD --
rust swift`, 38 lines): the compress refinements. `Connection::start` leaves the
handshake bound set, `open_at` clears it once welcomed and `attach` clears it on
the raw stream, instead of clearing and re-arming between `welcome` and
`bridged`; Desktop's three copies of the disconnect message are one
`ServerLink.disconnected`; the README paragraph stating the ten-second bound.
No wire or behavior change. The "Done when" results below were measured on this
tree.

**Remaining:**

1. Demo/review: the Desktop run below, including recovery after `lf server run`
   restarts. Headless steps cannot drive the app. The parent carries the
   behavior; either branch's build demonstrates it.
2. Publish. The declared step publishes this branch as a PR stacked on #1813,
   carrying only the refinements above. Folding them into #1813 as Jack did
   with the rest, and completing this Task against the parent's delivery, is
   the smaller alternative and Jack's call ([questions.md](questions.md)). No
   merge or Auto-Ship either way; LOO-593 lands first.

Owned elsewhere: retiring the revision-1 sniff (`refuse_revision_one` and its
test) belongs to the Task that retires revision 1; `lf session resume
--prepare` failing without a harness selection while saying `--agent` has no
effect is the command's own repair (the fixture names `-a claude`).

## Demo

Desktop runs a working session on one connection. With the Dev app open on a
repository, `lsof -U -p $(pgrep -f 'lf server run')` shows one Desktop socket
where there were dozens; `lf server status` prints `"revision":2` and
`"listening":true`. In the app: roadmap and Sessions render, a Session rename
lands, a Task save above 64 KiB succeeds, a terminal attaches, then
`lf server run` is restarted and the readings recover without relaunching the
app.

## Revision 2 on the wire

The protocol as built is documented in `rust/loopflow/src/server/README.md`
(messages, fields, limits, the revision-1 sniff, error codes, the null-id
`error` as the connection's last message); the plan does not repeat it. Decisions
the README states without history: `unsupported` was deleted by the concept
review because nothing produced it; `command.value` is stdout parsed as JSON only
when the write's `argv` carries `--json`; a revision-1 refusal keeps `instance`
so an old `client::status()` still reads it. Fixtures under
`tests/fixtures/dto/server/` cover every message (`bootstrap.json` deleted); no
DTO defaults in Rust or Swift.

## Architecture as built

**Rust server** (`service.rs`). `connection` sniffs one byte: revision 1 →
`refuse_revision_one`; otherwise `Framed<UnixStream, LinesCodec>` with the
32 MiB maximum, seeded with the sniffed byte. First message `status` → answer,
close. `hello` → `negotiate`, `welcome`, then `session`: a loop over incoming
messages with a bounded outgoing `mpsc` and writer task; a send that cannot
enqueue within five seconds ends the connection. A message that is not JSON, or
a second `hello`/`status`, is answered with a null-id `invalid` error and the
connection ends. `request`
spawns a task per id
and keeps its `AbortHandle`; reads and commands run through the existing
`spawn_blocking` + `journal::request` path. `Subscribe` answers `subscribed`,
then `watch` pulls frames into `reading` messages and forwards `checkpoint`
lines to `Subscription::request`, which owns validation and the 16 KiB limit.
The shared watch `Reader` owns a blocking runtime, so `Watch` is a drop guard
that releases the `Subscription` on a blocking thread however the task ends.
`cancel` aborts the task; connection end aborts every task; admitted mutations
complete because only the response path is aborted. `Attach` replies `bridged`
and hands the raw stream to `copy_bidirectional`, with no store read in its
path. `wire.rs` holds `ClientMessage`/`ServerMessage` (`tag = "type"`),
`Hello`, `ClientIdentity`, `Welcome`, `Status`, `Limits`, `Operation`, `Reply`,
`ErrorCode`, `REVISION`, `MIN_REVISION`, `MAX_MESSAGE`, `MAX_CONNECTIONS`.

**Rust client** (`client.rs`). `status()` is the pre-hello probe behind
`lf server status` and installation readiness: `listening`, `instance`,
`revision`, `ready`, `error`; it compares the executable hash and the revision
window, and reads a clean close before any answer as "older protocol".
`Connection::{open, open_at}` perform hello/welcome over a line-framed blocking
`UnixStream` within `HANDSHAKE` (ten seconds, shared with `status()`), then
clear the bound; `request`, `send`, `subscribe`, `reading`, `checkpoint` and
`set_read_timeout` serve `served_reads`, `machine_commands` and the relays.
`bridge(owner)` and `attach(path, owner)` do hello, attach, `bridged` (each
within the bound) and return the raw, unbounded stream for `transport.rs`.
A wait the bound cuts short reads "did not welcome the connection / open the
owner bridge within 10s".

**Swift.** `ServerProtocol.swift` mirrors the enums. `ServerClient` keeps the
`read/write/configuration/watch` signatures; a `ServerLinks` actor holds one
`ServerLink` per `ServerLocation`, opened on first use and reopened on the next
request after a loss with bounded backoff (100 ms doubling to 3.2 s). A
`ServerLink` has one reader thread with a `LineReader`, pending continuations by
id and `ServerSubscription`s feeding the existing `LineWatch`; writes are
serialized. `ServerLink.init` sets `SO_RCVTIMEO` to ten seconds for the
handshake and clears it before the reader thread starts, so a stalled open
fails and `ServerLinks` backs off. Task cancellation sends `cancel` and resumes with
`CancellationError`. Connection loss fails every pending request as outcome
unknown and every watch, which `WorkWatches` reopens. `ServerLocation` and
readiness are unchanged.

## Deleted — do not restore

Revision 1's one-operation protocol is gone on both sides: `receive`/`respond`,
`Operation::Bootstrap`, `VERSION`, `MAX_FRAME`, the `Request`/`Response`
envelopes, `client::connect(Operation)`, hand-rolled framing in the Rust tests,
Swift `ServerSocket`/`ServerRequestConnection`/`ServerWatch`/`ServerRequest`/
`ServerResponse`, `bootstrap.json` and the revision-1 doc sections. Compress
re-checked the tree on November 5: nothing on this list remains, and `watch.rs`'s
own `MAX_FRAME` is the watch's limit, not the protocol's. Survives: the socket,
admission, `journal::request` tracing, mutation dispatch, the owner bridge,
readiness, the relays, `WorkWatches`/`LineWatch` and `LocalServerFixture`.

## Forbidden outcomes

A revision-1 code path kept beside revision 2 beyond the one-byte sniff; a
`Reply::Read` string alongside `value`; a second mutation executor; a per-watch
socket in Swift; a reconnect token, replay cursor or event reducer; any
`#[serde(default)]` or Swift `??` on wire types; Process rows for requests.

## Done when (gate reruns these once, headless)

| Check | Command | Expect | Last result |
| --- | --- | --- | --- |
| Protocol unit tests | `cargo test -p loopflow --lib server::` | negotiation, revision-1 sniff, concurrent ids, cancel, status probe, malformed line closes, overlong line, stalled hello/attach fail in 10 s | 11/11 pass (compress, handshake bound) |
| Served parity and effects | `cargo test -p loopflow --test served_reads` | no Process rows; >64 KiB save; disconnect leaves the mutation applied | 5/5 pass, 122 s (compress) |
| Ownership routing over the client | `cargo test -p loopflow --test integration machine_commands::served_ownership` | pass without hand framing | pass (compress) |
| Swift mirror and client | `scripts/test_desktop.sh --filter 'ServerProtocolTests\|ServerClientTests\|WorkObservationTransportTests'` | fixtures decode with no defaults; cancel keeps the connection; disconnect fails all pending; a stalled handshake fails in 10 s and the next request reopens | ServerClientTests 6/6 pass (compress, handshake bound); others pass (compress) |
| Lint | `cargo fmt --check && cargo clippy --all-targets -- -D warnings` | clean | clean (compress, handshake bound) |
| Measurement | `LOOPFLOW_BOUNDARY_PERF=/tmp/rev2.json swift test --skip-build --package-path swift --filter ServerPerformanceTests` with `LF_*` unset | JSON written; `results.md` column; admitted connections for the workload = 1 | pass, 1 connection (realign) |

Quiet-host revision-2 medians sit within the spread between two baseline runs
([results.md](server-performance/results.md)): no local latency change. The win
is one connection per client; a regression is a sustained median rise.

## Phone (LOO-718) adopts

One connection per foreground session with the existing device grant on the
HTTP upgrade; one protocol message per WebSocket text frame, so the 64 KiB
byte-chunking and `MachineConnection.receiveResponse` reassembly are deleted;
the 20-second ping and revocation poll stay, now once per connection;
background suspension closes the connection, foreground reopens, re-sends
`hello` and re-subscribes, receiving complete readings. Its `remote.rs` admits
per connection and sets the same 32 MiB message cap. This Task does not edit
`/Users/jack/src/loopflow.mobile`; LOO-718 records this in its own handoff.

## Concept review — November 5, 2026 (headless)

Reviewed the built change against the parent's design
(`81cf2ee219:scratch/server-protocol-2.md`).
The promised experience holds: one connection, hello/welcome, requests by id,
complete readings on reconnect, terminal bytes on their own socket. Three
contract gaps were found and repaired the same day (decisions and check lines in
[questions.md](questions.md)): a null-id `error` is now the connection's last
message, `ErrorCode::Unsupported` is deleted, and both clients bound the
handshake at ten seconds as revision 1 did.

**Owned by a later Task:** Desktop still decodes twice. `read {value}` arrives
typed, `ServerClient.read` re-encodes it to text for `RegistryQuery.serve`, and
every Desktop reader decodes that text again; `commandOutput` discards
`command.value`. The string path is `RegistryQuery`'s shared shape across
subprocess and served reads, so collapsing it touches every Desktop reader and
follows LOO-593 and this Task. The wire types already carry what it needs.
`welcome.limits` and `capabilities` have no client consumer yet; they are the
design's affordance for a phone or shed. No open product choice.

**Second pass (same day, after the handshake-bound compress and realign).**
Walked the normal path, a stalled server and a restart through both clients
against the README: the bound is set once per open and cleared where each
caller has what it opened for (`open_at` after `welcome`, `attach` on the raw
stream after `bridged`, `ServerLink.init` before its reader starts); a stalled
`hello` or `attach` fails in ten seconds with the answer it waited for, and
Desktop's backoff reopens on the next request. The Swift mirror's `attach` and
`session_owner` cases have no Desktop sender (terminals reach the owner bridge
through the Rust relay); they stay as the mirror's whole-protocol contract
exercised by `ServerProtocolTests`. Nothing to change; no open product choice.
Where the remaining 38 lines land is Jack's delivery call, recorded in
[questions.md](questions.md), not a product choice.

## Risks

- **`welcome` is the handshake's only Work access.** Its Machine reading runs
  before the server answers, while `status` never opens Work. A registry that
  stalls fails every open with "did not welcome the connection within 10s;
  check `lf server status`", and that probe reports ready. A later slice can
  time-box the Machine read or move `machine` to an ordinary `read`; nothing
  measured so far shows the stall.
- **Head-of-line blocking** from a large reading on the shared connection:
  bounded per-connection queue; the measured workload shows none. If a Desktop
  run does, the answer is a second connection for bulk reads.
- **Swift reader-thread correctness** under cancellation and disconnect is the
  riskiest code; `ServerClientTests` cover cancel with a late answer, disconnect
  failing all pending work and reopening, and a server-ended subscription
  failing its watch alone.
- **Stack hygiene**: the parent's design and earlier notes are read at the
  parent commit ([loo-593-parent.md](loo-593-parent.md)); `server-performance/`
  and the Rex comparison stay here. LOO-593's land removes all of them from main.
