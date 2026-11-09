# Connect to the local Loopflow server

```sh
lf server status          # readiness, socket, protocol revision, incarnation and connected clients as JSON
lf server describe --json # the JSON Schema of the protocol this build speaks
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

## Local protocol, revision 3

One connection per client over the same-user Unix socket. Every message is one
JSON object per line (`serde_json` never emits a raw newline). Messages are
capped at 32 MiB and client connections at 64; terminal bridges are separate
connections that never count against that cap. A connection has five seconds to
send its first message and five seconds to take any message the server sends. The
socket and its directory are private; remote proxies must never forward this
endpoint. Remote admission is outside this revision.

`wire.rs` owns the Rust types; `swift/Loopflow/Services/ServerProtocol.swift`
mirrors them. Executable fixtures live in `tests/fixtures/dto/server/`.
`nc -U machine.sock` is a debugging tool: type `{"type":"status"}`.

`lf server describe --json` prints the protocol as one JSON Schema, generated
from those types by `schema.rs`; bare `lf server describe` summarizes it. It
reads no store and needs no running server, so a client asks the `lf` on its host
rather than the connection. The root accepts any message either side sends;
`$defs` holds `ClientMessage`, `ServerMessage`, `Hello`, `Welcome`, `Read`,
`Write`, `ServerOperation` and every type they reach, and `x-revision` and
`x-min-revision` are this build's range. Each operation's kind is a `const` as
on the wire, and `x-result` beside it names what `response.result` carries. Where a recorded exchange exists, `examples` holds the request's operation and `x-result-example` what it answered, from the same fixtures the tests check. A
subscription frame's `body` refers to `$defs/Untyped`, because its shape is the
`x-result` of the read that opened it. The serialize contract keeps a nullable
field required, as both sides send it.

Rust validates every fixture against its definition, every `response` fixture
against its operation's `x-result` and, in `served_reads`, every live read.
`tests/fixtures/dto/server_schema.json` is the same document checked in. A Rust
test fails when it is stale and names the regeneration command:

```sh
cargo run -p loopflow --bin lf -- server describe --json > tests/fixtures/dto/server_schema.json
```

`ServerProtocolTests` reads that file: every operation Desktop encodes must have
a described kind with exactly its fields, every client fixture must be encoded
by a test and every server fixture must decode.

```text
→ {"type":"hello","revision":3,"min_revision":3,"client":{"kind":"desktop","platform":"macos","name":"Loopflow","version":"0.15.1"}}
← {"type":"welcome","client_id":"…","revision":3,"server_revision":3,"min_revision":3,"instance":"…","build":"0.15.1","executable_sha256":"…","machine":{…},"limits":{"max_message":33554432,"max_connections":64}}
→ {"type":"request","id":"a","operation":{"kind":"plan.list","directory":"/","wave":null,"task":null,"all":true},"select":null}
→ {"type":"subscribe","id":"b","operation":{"kind":"session.list","directory":"/repo","headless":false,"after":null,"limit":null},"select":["id","title","state"],"resume":null}
← {"type":"response","id":"a","trace_id":"…","result":{…}}
← {"type":"response","id":"b","trace_id":"…","result":null}
← {"type":"reading","id":"b","checkpoint":"opaque-reading-token","frame":{"answers":null,"unavailable":null,"body":{…}}}
→ {"type":"checkpoint","id":"b","checkpoint":3}
← {"type":"reading","id":"b","checkpoint":"opaque-reading-token","frame":{"answers":3,…}}
→ {"type":"cancel","id":"b"}
→ {"type":"request","id":"c","operation":{"kind":"task.run","directory":"/repo","issue":"LOO-1","from":null,"agent":null,"approve":false},"select":null}
← {"type":"response","id":"c","trace_id":"…","result":{"state":"running","issue":"LOO-1","run":"code from start","graph_process":"…","log":"…"}}
→ {"type":"subscribe","id":"d","operation":{"kind":"task.wait","issue":"LOO-1","until":"submitted"},"select":null,"resume":null}
← {"type":"reading","id":"d","checkpoint":"another-token","frame":{"answers":null,"unavailable":null,"body":{"reached":true,"task":{…}}}}
← {"type":"ended","id":"d"}
```

The first message is `hello` with the revisions the client speaks. The server
chooses the newest revision both sides speak and answers `welcome` with it, a
server-assigned `client_id`, the server's own range, its incarnation (`instance` changes with every start), build,
executable SHA-256, the Machine reading (`null` while the registry is
unavailable or uninitialized) and limits. What the server can do is its
revision and `lf server describe`, not a list in the welcome. With no revision in
common it answers `error {code: incompatible}` naming the side to update in
its message and in `data.update` (`client` or `server`), then closes. This build speaks revision 3 only; the shape lets a later server speak
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
operation's value: what its CLI spelling prints with `--json`, a ticket
`{ticket, owner_lf_process_id, expires_in_ms}` for `session.attach`, and null
once a subscription is confirmed.
A completed request's ID can be reused immediately after its response or error. A
subscription keeps its ID after acknowledgement and readings, until `ended`,
an error or cancellation.
`trace_id` names the request in `logs/lf.log`. `cancel {id}` forgets a request's
answer or ends a subscription; a write already dispatched still completes and
answers nothing. Closing the connection cancels every subscription and releases
every wait; admitted mutations finish. An `error` with a null `id` is about the
connection and is its last message: the server closes after `busy` at capacity
(nothing ran; retry shortly) and after `invalid` for a malformed line or a
repeated `hello`/`status`. A well-formed `request` or `subscribe` whose
operation this build does not accept fails alone: `invalid` with its own `id`
and `data.kind`, and the connection stays open, so a newer client loses only
that request. `data` is structured detail or null. Clients branch
on `code`; only `outcome_unknown` means the request may have taken effect:

| `code` | Meaning | `data` |
| --- | --- | --- |
| `incompatible` | No revision in common | `{server: {revision, min_revision}, update}` |
| `invalid` | Malformed message, an operation this build does not have, or refused arguments, such as subscribing to a read with no subscription | `{kind}` for a refused operation, else null |
| `not_found` | The named owner, subscription, repository checkout, view member or record does not exist | what was named: `{owner_lf_process_id}`, `{id}`, `{repo_id}`, `{expression}` |
| `conflict` | The request contradicts current state: a still-unanswered request ID | `{id}` |
| `unavailable` | A dependency is missing or failed: the Machine registry, a subscription, a read | `{resource}` when one is named |
| `outcome_unknown` | The operation may have run; inspect state before retrying | null |
| `busy` | Capacity reached, or a subscription fell behind its checkpoints; retry shortly | `{max_connections}` at capacity |

A shared operation names its code by returning `engine::Error` inside its
error; anything else a read returns is `unavailable`.

After editing a Task's checkout, request its changed files on the same connection:

```json
{"type":"request","id":"changes","operation":{"kind":"task.changes","issue":"LOO-1","base":"parent","head":null},"select":null}
```

The response reflects Git state read for this request, even while subscriptions
are active. One-shot reads and served writes consult Git afresh. Only subscription
readings reuse retained Git answers, shared across subscriptions so listing
checkouts side by side stays fast. Git observations are not an atomic snapshot
of a checkout being edited.

Every operation names its `kind` as a dotted resource and verb, beside its
fields. `lf/commands/read.rs` (`Read`) and `lf/commands/write.rs` (`Write`) own
the shared kinds; `wire::ServerOperation` owns the server's own.

Rust variants use the kind's PascalCase spelling (`ProcessList`); Swift cases
use lowerCamelCase (`processList`). Dots and underscores separate words. Each
case names one operation, including distinct claim/unassign and pin/unpin writes.

| API kind | CLI | Engine |
| --- | --- | --- |
| `task.get` | `lf task show <issue>` | read |
| `task.list` (`closed: true`) | `lf task list --closed --wave <wave>` | read |
| `task.changes` | `lf task changes <issue>` | read |
| `task.diff`, `task.file` | `lf task diff`, `lf task file` | read |
| `task.context` | `lf task context <issue>` | read |
| `task.runs` | `lf task runs <issue>` | read |
| `task.wait` | `lf task wait <issue> --until <state>` | `engine::task` |
| `task.create` | `lf task create --session <id>` | write |
| `task.run` | `lf task run <issue>` | `engine::task` |
| `task.ship` | `lf ship` | `engine::task` |
| `task.hold` | `lf task hold <issue>` | `engine::task` |
| `task.steer` | `lf task steer <issue> <message>` | `engine::task`; the thread entry the store saves as the Task's steer |
| `task.report` | `lf task report <issue> <message>` | `engine::task` |
| `task.interrupt` | `lf task interrupt <issue>` | `engine::task` |
| `task.update` | `lf task update <issue>` | write |
| `task.move`, `.claim`, `.unassign`, `.save`, `.complete`, `.reopen`, `.abandon` | `lf task <verb>` | write |
| `session.list` | `lf session list` | read |
| `session.attach` | `lf session attach` (Desktop panes' relay; ordinary CLI attachment reaches the owner's socket directly) | server |
| `session.usage` | `lf session usage` (`lf usage`) | read |
| `session.history` | `lf session history` (`lf history`) | read |
| `session.retention` | `lf session retention` | read |
| `session.rename`, `.bind`, `.resume`, `.new` | `lf session <verb>`, `lf resume` | write |
| `session.steer` | `lf [--wave <wave>] session steer [<id>] <message>` | write; `engine::session::steer` |
| `wave.list`, `wave.graph` | `lf wave list`, `lf wave graph show` | read |
| `wave.plan` | `lf wave plan <wave>` | read |
| `wave.create` | `lf wave create` | write |
| `wave.sync` | `lf wave sync <wave>` | write |
| `wave.plan.bind`, `.connect`, `.reteam` | `lf wave plan bind`, `connect`, `reteam` | write; `engine::wave::planning_bind`, `planning_connect`, `planning_reteam` |
| `plan.list` | `lf plan list` | read |
| `repo.plan` | `lf repo plan` | read |
| `repo.people`, `.tokens`, `.path` | `lf repo people`, `lf repo tokens`, `lf repo path` | read |
| `repo.release` | `lf repo release run [<version>] [-t <target>]`; the other `release` verbs are its steps, CLI only | write, detached; `engine::repo::release` |
| `repo.ci` | `lf repo ci [--since <since>] [--wave <wave>] [--repo <repo>]` | read; `engine::repo::ci` |
| `graph.list`, `graph.customize` | `lf graph list`, `lf graph customize` | read, write |
| `graph.run` | `lf graph <name>` (`lf -b`, detached; `task.run` keeps the Task spelling) | write; `engine::graph::run` (the Process carrying the Graph) |
| `process.get` | `lf process show <id>` | read |
| `machine.self`, `.list`, `.doctor` | `lf machine self`, `lf machine list`, `lf machine doctor` (`lf doctor`) | read |
| `process.list` | `lf process list` | read |
| `work.resolve` | `lf open <url> --link`; a `work` noun would shadow the builtin `work` skill | read |
| `config.catalog`, `.settings`, `.get`, `.explain` | `lf config catalog`, `lf config settings`, `lf config get`, `lf config explain` | `engine::config` |
| `config.set`, `.unset` | `lf config set`, `lf config unset` (`lf set`, `lf unset`) | `engine::config` |
| `config.theme` | none; Appearance's theme file | `engine::config` |
| `pr.publish`, `.open`, `.submit`, `.arm`, `.abandon`, `.reconcile`, `.request_review` | `lf pr publish`, `lf pr open`, `lf pr submit`, `lf pr arm`, `lf pr abandon`, `lf pr reconcile`, `lf pr request-review` | write; `engine::pr::publish`, `open`, `submit`, `arm`, `abandon`, `reconcile`, `request_review` |
| `pr.land` | `lf pr land` (`lf land`) | write; `engine::pr::land` (detached; the Process carrying the landing) |
| `pr.checks`, `.reviews` | `lf pr checks`, `lf pr reviews` | read; `engine::pr::checks` (no `--subscribe`), `reviews` |
| `checkout.commit`, `.sync`, `.create` | `lf commit`, `lf sync`, `lf wt create` | write; `engine::checkout::commit`, `sync` (detached unless `--plan`, `--continue`, `--abort`), `create` |
| `account.list`, `.next` | `lf account [<login|provider>]`, `lf account next` | read; `engine::account::list`, `next` |
| `account.connect`, `.disconnect`, `.set`, `.use`, `.pause`, `.unpause`, `.redeem_reset` | `lf account connect`, `disconnect`, `set`, `use`, `pause`, `unpause`, `redeem-reset` | write; `engine::account::connect`, `disconnect`, `set`, `activate`, `pause`, `redeem_reset` (none prompts) |
| `skill.run` | `lf skill <name>` (`lf -b`, detached) | write; `engine::skill::run` (the Process carrying the run) |
| `skill.list` | `lf list [<path>…]` | read; `engine::skill::list` |
| `desktop.portfolio`, `.portfolio.set` | `lf desktop portfolio` (`lf portfolio`) | read, write |
| `desktop.layout`, `.layout.save` | `lf desktop layout <work> [--save]` | read, write |
| `desktop.view.list`, `.view.get` | `lf desktop view list`, `lf desktop view show` | read |
| `desktop.dashboard.*` | `lf desktop dashboard query`, `pin`, `unpin`, `rename`, `hide`, `show`, `move`, `resize`, `slots` | write |
| `desktop.<verb>` | `lf desktop enter`, `send`, `send-key`, `capture`, `zoom`, `focus`, `close`, `reopen`, `move`, `resize`, `browse`, … | CLI only, executed by the app |

"CLI handler; not served" is an operation only the CLI dispatcher runs today;
its kind fixes the name a served version keeps.

The nouns are Loopflow's primitives (Wave, Task, Graph, Session, Process,
Machine, Repository, Work, PR, Checkout, Skill, Account), plus `config` for
settings, `desktop` for the app and `planning` for every Wave's plan at once;
nothing gets a namespace of its own for a client.

`desktop.*` is Desktop's own arrangement state: which repositories its windows
show, saved pane layouts and Dashboards. It is served for Desktop and is not
part of what a phone or bot client is offered.

- `config.*` is `engine::config`, which `lf config` calls too: `config.catalog`
  lists the typed settings, `config.settings {directory, wave, skill, graph,
  step}` resolves them with their sources, and `config.get`, `config.set
  {value}`, `config.unset` and `config.explain` take `{destination, key}`, where
  a destination names an owner (`personal_global`, `shared_repo {repo}`,
  …), never a file plus a scope. `config.theme {path, value}` is Appearance's
  theme file. A failed or uncertain save is never replayed; setting values are
  omitted from request telemetry.
- A read uses the existing `Read` shape and answers with the JSON the command
  prints, serialized once. A command that prints bare text
  (`repo path`) answers a JSON string. Every shared read is
  supported. Repository IDs resolve on the destination with `repo.path`;
  paths in local reads are destination paths. `desktop.view.get` returns the existing
  WorkViewPlan, including the recorded primary Session; planning membership alone
  cannot select one. A missing primary stays absent. Attach to it with
  `session.attach`; no owner lookup comes first.
- `select` on a `request` or `subscribe` keeps only those dotted paths of a
  read's answer: `["task.identifier","pr.url"]` keeps each path's ancestors as
  objects, applies a path through an array to each element, and leaves a path
  the answer lacks absent. `null` keeps the whole answer; a write with a
  `select` is `invalid`. A subscription compares selected readings, so a
  change outside the selection sends nothing; `task.wait` always keeps
  `reached`, which ends it. `lf <read> --json --select task.identifier,pr.url`
  prints the same projection; only the reads that take `--subscribe` accept
  the flag. `select` serves untyped clients (agents, scripts, the CLI). A
  typed client such as the Swift app decodes every DTO strictly with no
  defaults, so a selected answer would not decode: it sends `null`, and gets
  a smaller payload only through a read with its own DTO.
- `subscribe` takes a read with a subscription: `desktop.view.list`,
  `session.list` without `limit`, `task.get`, `task.wait`, `session.history`,
  `plan.list`, `repo.plan` and `process.list`. It answers
  a null `response`, then `reading {id,checkpoint,frame}` messages wrapping
  the existing complete `answers/unavailable/body` subscription frame. `checkpoint
  {id,checkpoint}` asks for a reading now; the next reading's `answers` carries
  that number, changed or not. A subscription's arguments are fixed: new
  arguments are a new subscription with a complete first reading. A subscription that
  fails sends its `unavailable` reading, then `error {id, code: unavailable}`
  ends the subscription; the connection stays open. A read with no subscription is
  `invalid`. On reconnect, supply the last applied reading's checkpoint to resume the
  same read without receiving an unchanged complete reading; see
  [Reconnect a watch](#reconnect-a-watch). `task.wait {issue,
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
  (`{issue, stopped}`), `task.steer` and `task.report {message}` save to the
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
- `session.attach {session, owner_lf_process_id}` answers a ticket
  `{ticket, owner_lf_process_id, expires_in_ms}` on an ordinary client
  connection, which stays a client. A separate bridge connection sends
  `{"type":"attach","ticket":"<ticket>"}` as its only JSON message; the server answers
  `bridged {owner_lf_process_id}`, and the connection then speaks the unchanged
  [binary owner protocol](../transport/README.md) in `transport/wire.rs`. A
  ticket is 16 random bytes as 22 base64url characters, opens one bridge, lasts
  60 seconds and is bound to the requesting client's Unix peer UID. Presenting
  a ticket consumes it even when the UID does not match. An unknown,
  spent, expired or foreign ticket is `not_found` with a null `id` and closes
  the connection. Bridges last only as long as their Session owners and do not
  count against the 64 client slots.
  The owner is the LfProcess holding the Session's terminal (`lf session
  serve`), never the provider's AgentProcess beneath it. The server resolves the
  Session's current owner when it issues the ticket; resolution never starts a
  provider. A Session with no running owner is `not_found {session}`; an owner
  whose endpoint is gone is `not_found {owner_lf_process_id}`: it ended, at
  issue or when the ticket is presented. When the owner ends, the bridge closes,
  and attaching again by the same Session reaches its next owner, as after `lf
  install`. A client that reconnects only to the owner it had passes it as
  `owner_lf_process_id`; a Session now owned by another LfProcess is then
  `not_found {owner_lf_process_id, current_lf_process_id}`. Desktop's relays do
  this; `lf session resume` and `server::client::attach` perform both steps.
  Each terminal has its own connection, so readings never wait behind terminal
  bytes and a control failure never closes a bridged terminal. The owner alone grants input/geometry,
  handles bounded output continuation and revokes stale control. No automatic
  owner replacement or raw-input replay. Desktop automatically reconnects the
  exact owner with bounded backoff, keeping connection status in native chrome
  outside terminal bytes. Recovery uses Resume for byte streams or a fresh
  terminal screen and requests free claims without replacement. Newer viewers
  retain input and geometry. Unavailable keeps the last screen marked stale;
  Retry reconnects without preparing or resuming a provider. A connection
  failure alone never means the Session ended.

## Reconnect a watch

After reconnecting and completing `hello`/`welcome`, resume the `session.list`
watch above with the checkpoint from its last applied reading:

```text
→ {"type":"subscribe","id":"b2","operation":{"kind":"session.list","directory":"/repo","headless":false,"after":null,"limit":null},"select":["id","title","state"],"resume":"opaque-reading-token"}
← {"type":"response","id":"b2","trace_id":"…","result":null}
```

If the selected list is unchanged, no `reading` follows until a change or a
confirmation request. Keep the reading and its opaque `reading.checkpoint`
together; resume with the same operation, arguments and `select`. A new request
ID is fine. Use the token the server returned, not the example's placeholder.

Changed or unknown checkpoints, including those from a restarted server, get a
complete current reading as the new baseline. Start with `resume: null` if the
client discarded its reading or changed its query. Resume covers current state;
intermediate changes are not replayed and readings are never patches. Read
timestamps alone do not count as changes. Tokens fingerprint the operation,
arguments, selection and reading within one server incarnation, requiring no
retained payload or disconnected watch.

The numeric `checkpoint` request and `frame.answers` confirm a live read; they
are separate from the opaque resume token. Confirmations always get a reading,
even when unchanged. Unavailable readings are always sent, and a terminal
`task.wait` still sends `ended`, even when its unchanged final reading is omitted.

Desktop holds one connection per server location for the app's lifetime, opened
on first use and reopened on the next request after a loss with bounded backoff
(100 ms doubling to 3.2 s). Its subscriptions share that connection. Losing it
fails every pending request as outcome unknown. A watch with a received reading
keeps its stream and queued frames, reconnects, and resumes from that reading's
token. A complete reading replaces it when the token is unknown. The newest
unanswered confirmation retries and covers earlier requests; writes never retry.
Cancelling the watch cancels recovery. A watch without a baseline, a server refusal, or a slow
consumer that overflows its frame queue ends the stream; the window then opens a
fresh watch. If reopening the connection fails, the stream also ends with that
error so the window reports unavailability and retries.

Every client gives the server ten seconds to answer `status`, `hello`,
`session.attach` or `attach`; a server that accepts and stalls fails the open
with that diagnosis and the caller's backoff retries. A welcomed session's waits and the bridged
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

## Remote attach contract

Planned for the authenticated endpoint in [Phone access](../../../../docs/server.md#phone-access);
this build supports local attachment only. Owner selection, terminal control,
ticket consumption and rejection follow `session.attach` above.

After `welcome`, request `session.attach` on the authenticated client connection.
Use a Session ID from `session.list`; `owner_lf_process_id: null` selects its
current running terminal owner:

```text
→ {"type":"request","id":"attach-1","operation":{"kind":"session.attach","session":"<session ID>","owner_lf_process_id":null},"select":null}
← {"type":"response","id":"attach-1","trace_id":"<trace ID>","result":{"ticket":"<ticket>","owner_lf_process_id":"<owner ID>","expires_in_ms":60000}}
```

Open a separate bridge connection, authenticate it through the remote endpoint,
and send `attach` as its first message, without a `hello` exchange:

```text
→ {"type":"attach","ticket":"<ticket>"}
← {"type":"bridged","owner_lf_process_id":"<owner ID>"}
```

Both connections must authenticate with the same Tailscale node stable ID and
user ID; the same account on another device does not match. A ticket, `hello`
name, `client_id`, login text or proxy's Unix UID cannot replace that authentication.
Local and remote tickets are not interchangeable. The 60-second deadline includes
host sleep; expiry does not close an open bridge.

Request a fresh ticket before reconnecting.
