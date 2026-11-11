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

## Local protocol, revision 4

One connection per client over the same-user Unix socket. Every message is one
JSON object per line (`serde_json` never emits a raw newline). Messages are
capped at 32 MiB and client connections at 64; terminal bridges are separate
connections that never count against that cap. A connection has five seconds to
send its first message and five seconds to take any message the server sends. The
socket and its directory are private; remote proxies must never forward this
endpoint. Remote admission is outside this revision.

`wire.rs` owns the Rust types; `swift/Loopflow/Services/ServerProtocol.swift`
mirrors them. Executable fixtures live in `rust/loopflow/tests/fixtures/dto/server/`.
`nc -U machine.sock` is a debugging tool: type `{"type":"status"}`.

`lf server describe --json` prints the protocol as one JSON Schema, generated
from those types by `schema.rs`; bare `lf server describe` summarizes it. It
reads no store and needs no running server, so a client asks the `lf` on its host
rather than the connection. The root accepts any message either side sends;
`$defs` holds `ClientMessage`, `ServerMessage`, `Hello`, `Welcome`, `Read`,
`Write`, `ServerOperation` and every type they reach, and `x-revision` and
`x-min-revision` are this build's range. Each operation's kind is a `const` as
on the wire, and `x-result` beside it names what `response.result` carries.
For a write, that schema derives from the Rust type the operation returns.
For example, `task.ship` returns `TaskShip`: its `state` is `shipping`,
`already_shipping` or `at_end`, with the fields described by that variant.
`lf --task <issue> ship --json` prints the same result. Use the operation's
`x-result` to decode it; `response.result` adds no extra result-type tag.

Where a recorded exchange exists, `examples` holds the request's operation and
`x-result-example` what it answered, from the same fixtures the tests check. A
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
→ {"type":"hello","revision":4,"min_revision":4,"client":{"kind":"desktop","platform":"macos","name":"Loopflow","version":"0.15.1"}}
← {"type":"welcome","client_id":"…","revision":4,"server_revision":4,"min_revision":4,"instance":"…","build":"0.15.1","executable_sha256":"…","machine":{…},"limits":{"max_message":33554432,"max_connections":64,"max_open":256,"max_retained_bytes":67108864}}
→ {"type":"request","id":"a","operation":{"kind":"plan.list","directory":"/","wave":null,"task":null,"all":true},"select":null}
→ {"type":"subscribe","id":"b","operation":{"kind":"session.list","directory":"/repo","headless":false,"after":null,"limit":null},"select":["id","title","state"],"resume":null}
← {"type":"response","id":"a","trace_id":"…","result":{…}}
← {"type":"response","id":"b","trace_id":"…","result":null}
← {"type":"reading","id":"b","checkpoint":"opaque-reading-token","frame":{"answers":null,"unavailable":null,"body":{…}}}
→ {"type":"checkpoint","id":"b","checkpoint":3}
← {"type":"reading","id":"b","checkpoint":"opaque-reading-token","frame":{"answers":3,…}}
→ {"type":"cancel","id":"b"}
→ {"type":"request","id":"c","operation":{"kind":"task.run","directory":"/repo","issue":"LOO-1","graph":null,"from":null,"agent":null,"approve":false},"select":null}
← {"type":"response","id":"c","trace_id":"…","result":{"state":"running","issue":"LOO-1","run":"code from start","ships":false,"graph_process":"…","log":"…"}}
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
its message and in `data.update` (`client` or `server`), then closes. This build speaks revision 4 only; the shape lets a later server speak
two. A revision-1 client's four-byte length prefix starts below `0x20`; the
server answers it a revision-1 framed `incompatible` error carrying `instance`
and closes. That sniff is the whole compatibility layer and goes with revision 1.

`status` before `hello` is the readiness probe: `status {revision, min_revision,
instance, executable_sha256, clients}`, then the server closes. `lf server status` and
installation use it so readiness never opens Work or speaks the full protocol;
`lf server status` also reports `listening` (a server accepted the connection),
`revision` and `clients`: each welcomed connection still open, oldest first, as
`{id, client, connected_at, requests, open, retained_bytes}`, where `client` is the identity its
`hello` gave and `requests` counts its requests, subscriptions included. The
`open` and `retained_bytes` fields report current admitted work and retained-byte
accounting, including pending producers. The
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
on `code`; `outcome_unknown` means the request may have taken effect:

| `code` | Meaning | `data` |
| --- | --- | --- |
| `incompatible` | No revision in common | `{server: {revision, min_revision}, update}` |
| `invalid` | Malformed message, an operation this build does not have, or refused arguments, such as subscribing to a read with no subscription | `{kind}` for a refused operation, else null |
| `not_found` | The named owner, subscription, repository checkout, view member or record does not exist | what was named: `{owner_lf_process_id}`, `{id}`, `{repo_id}`, `{expression}` |
| `conflict` | The request contradicts current state: a still-unanswered transport ID or a creation ID reused with different content | `{id}` or `{request_id}` |
| `unavailable` | A dependency is missing or failed: the Machine registry, a subscription, a read | `{resource}` when one is named |
| `outcome_unknown` | The operation may have run; inspect its creation receipt when available, otherwise the resource | creation details when known, otherwise null |
| `busy` | Admission capacity reached, an oversized read answer, or retained subscription capacity is exhausted | `{max_connections}` at connection capacity; `{limit, max}` for request admission; `{limit: "retained_bytes"}` for an answer that exceeds the message limit |

A shared operation names its code by returning `engine::Error` inside its
error; anything else a read returns is `unavailable`.

After editing a Task's checkout, request its changed files on the same connection:

```json
{"type":"request","id":"changes","operation":{"kind":"task.changes","issue":"LOO-1","base":"parent","head":null},"select":null}
```

The response uses the server's latest observed Git state. Served requests and
subscriptions share observations of process liveness, checkout Git state, PR/CI
status and repository files. The server refreshes these facts on a loop, so an
external change appears after it is observed. Ordinary CLI reads call the same
observation functions directly and need no running server. Served writes still
consult Git afresh. Git observations are not an atomic snapshot of a checkout
being edited.

`observe.rs` owns the fact observers and in-memory cache. Bounded workers inherit
the reading scope; a mutation runs outside it. File and process observations,
Git commands and GitHub requests refresh independently, so a slow provider does
not hold up local observations. Facts no longer read expire from memory.

Observed answers and their revisions live only in process memory, never in
SQLite; server replacement starts with new observations. CLI `--subscribe`
readings use the same cache in their own process. A subscription follows the
observed facts it reads alongside the existing store revisions (all domains
except usage). Changes to those facts or revisions wake it; an unchanged
selected answer sends no new reading. External dependencies come from the read
itself, without a separate clock, checkout or probe policy for each operation.
Database-derived joins retain their transactional snapshot; external
observations do not share that transaction.

Every operation names its `kind` as a dotted resource and verb, beside its
fields. `lf/commands/read.rs` (`Read`) and `lf/commands/write.rs` (`Write`) own
the shared kinds; `wire::ServerOperation` owns the server's own.

Rust variants use the kind's PascalCase spelling (`ProcessList`); Swift cases
use lowerCamelCase (`processList`). Dots and underscores separate words. Each
case names one operation, including distinct claim/unassign and pin/unpin writes.

| API kind | CLI | Engine |
| --- | --- | --- |
| `task.get` | `lf task show <issue>` | read |
| `task.detail` | `lf task detail <issue>` | read |
| `task.list` (`closed: true`) | `lf task list --closed --wave <wave>` | read |
| `task.changes` | `lf task changes <issue>` | read |
| `task.diff`, `task.file` | `lf task diff`, `lf task file` | read |
| `task.context` | `lf task context <issue>` | read |
| `task.runs` | `lf task runs <issue>` | read |
| `task.wait` | `lf task wait <issue> --until <state>` | `engine::task` |
| `task.create` | `lf task create [--session <id>] [--run] --request-id <id>` | write |
| `creation.get` | `lf server receipt <request-id> --json` | read; no creation or launch |
| `task.run` | `lf task run <issue>` | `engine::task` |
| `task.ship` | `lf ship` | `engine::task` |
| `task.hold` | `lf task hold <issue>` | `engine::task` |
| `task.steer` | `lf task steer <issue> <message>` | `engine::task`; the thread entry the store saves as the Task's steer |
| `task.report` | `lf task report <issue> <message>` | `engine::task` |
| `task.interrupt` | `lf task interrupt <issue>` | `engine::task` |
| `task.update` | `lf task update <issue>` | write |
| `task.move`, `.claim`, `.unassign`, `.save`, `.complete`, `.reopen`, `.abandon` | `lf task <verb>` | write |
| `session.list` | `lf session list` | read |
| `session.card` | `lf session card <id>` | read |
| `session.attach` | `lf session attach` (Desktop panes' relay; ordinary CLI attachment reaches the owner's socket directly) | server |
| `session.usage` | `lf session usage` (`lf usage`) | read |
| `session.history` | `lf session history` (`lf history`) | read |
| `session.retention` | `lf session retention` | read |
| `session.rename`, `.bind`, `.resume`, `.new` | `lf session <verb>`, `lf resume` | write |
| `session.steer` | `lf [--wave <wave>] session steer [<id>] <message>` | write; `engine::session::steer` |
| `session.status` | `lf session observe-status <id> --terminal <terminal> --stream <stream> --sequence <n>`, the snapshot on stdin | write; one Program Status reading from the terminal showing a Session. A stream's first reading replaces any earlier stream; a late reading never replaces a newer one |
| `wave.list`, `wave.graph` | `lf wave list`, `lf wave graph show` | read |
| `wave.plan` | `lf wave plan <wave>` | read |
| `wave.create` | `lf wave create` | write |
| `wave.rename`, `.remove` | `lf wave rename`, `lf wave remove` | write |
| `wave.machine` | `lf --wave <wave> set machine <machine>` | write |
| `wave.sync` | `lf wave sync <wave>` | write |
| `wave.plan.bind`, `.connect`, `.reteam` | `lf wave plan bind`, `connect`, `reteam` | write; `engine::wave::planning_bind`, `planning_connect`, `planning_reteam` |
| `plan.list` | `lf plan list` | read |
| `plan.board` | `lf plan board` | read |
| `repo.plan` | `lf repo plan` | read |
| `repo.people`, `.tokens`, `.path` | `lf repo people`, `lf repo tokens`, `lf repo path` | read |
| `repo.release` | `lf repo release run [<version>] [-t <target>]`; the other `release` verbs are its steps, CLI only | write, detached; `engine::repo::release` |
| `repo.ci` | `lf repo ci [--since <since>] [--wave <wave>] [--repo <repo>]` | read; `engine::repo::ci` |
| `graph.list`, `graph.customize` | `lf graph list`, `lf graph customize` | read, write |
| `graph.run` | `lf graph <name>` (`lf -b`, detached; `task.run` keeps the Task spelling) | write; `engine::graph::run` (the Process carrying the Graph) |
| `process.get` | `lf process show <id>` | read |
| `process.history` | `lf history show <process-id>` | read; rendered history |
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
settings, `desktop` for the app and `plan` for every Wave's plan at once;
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
  the existing complete `answers/unavailable/body` subscription frame. Typed
  [events](#events-alongside-readings) can arrive on the same subscription. `checkpoint
  {id,checkpoint}` asks for a reading now; the next reading's `answers` carries
  that consumer's newest requested number, changed or not; numbers need not
  increase. Concurrent checkpoints coalesce. The answer comes from a read begun
  after the request. Identical operation, arguments, selection and server context
  share one reader across connections. Each consumer has its own acknowledgement,
  checkpoints and latest delivery; a slow consumer does not hold up siblings.
  Cancelling the last consumer removes the demand, including queued setup or
  pulls. Already admitted reads may finish and release their resources. Reader
  execution remains independent of the server's blocking pool. These readers
  own no provider or terminal lifetime. A subscription's arguments are fixed: new
  arguments are a new subscription with a complete first reading. A subscription that
  fails sends its `unavailable` reading, then `error {id, code: unavailable}`
  ends the subscription; the connection stays open. A read with no subscription is
  `invalid`. On reconnect, supply the last applied reading's checkpoint to resume the
  same read without receiving an unchanged complete reading; see
  [Reconnect a watch](#reconnect-a-watch). `task.wait {issue,
  until}` (`submitted` or `terminal`) reads `{reached, task}` whenever the Task
  changes; after the reading whose `reached` is true and its events, the server
  sends `ended {id}` and nothing more for it. Requested once, it is that reading now, as
  `lf task wait --timeout 0s --json` prints it.
- The core loop is `engine::task`, which the CLI handlers call too.
  `task.run {directory, issue, graph, from, agent, approve}` and `task.ship
  {directory, issue}` outlive the request, so the server starts their
  command (`lf [-b] [-a <agent>] task run …`, `lf --task <issue> ship`)
  detached under its own execution owner and answers what that process
  reports once it knows: `{state: running, ships, graph_process, log}` when the
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
  standard output. `task.create {directory,request_id,title,request,wave,graph,session,run}`
  files a Task or adopts the named Session; `run` requests its Graph launch.
  A null `wave` selects the repository. Both paths return the same creation
  receipt, described below. A write whose Work
  lives on another Machine (`task.run`, `task.ship`, `task.hold`,
  `task.claim`, `task.unassign`, `session.resume`) runs its `lf … --json`
  spelling there over SSH and returns what that prints, which is the
  destination's value; it never falls back to local execution. Requests create no Process rows; diagnostics the shared
  functions print go to the request's log. A refusal the shared code names
  (an empty name is `invalid`, a stale Dashboard revision `conflict`, a missing
  record `not_found`) answers that code; any other failure is
  `outcome_unknown`, since the write may have taken partial effect. A lost
  response means outcome unknown; inspect state before retrying. Transport IDs
  correlate diagnostics and are **not** durable deduplication receipts. Only
  `task.create` and `session.new` have the creation retry contract below.

## Terminal attachment

`session.attach {session, owner_lf_process_id}` answers a ticket
`{ticket, owner_lf_process_id, expires_in_ms}` on an ordinary client connection.
A separate bridge sends `{"type":"attach","ticket":"<ticket>"}` as its only
JSON message. After `bridged {owner_lf_process_id}`, it speaks the
[binary owner protocol](../transport/README.md). A ticket is 16 random bytes
as 22 base64url characters, opens one bridge and lasts 60 seconds. It is bound
to the requesting client's Unix peer UID. Every redemption attempt consumes
the ticket, including a UID mismatch. Remote identity and sleep-aware expiry
are planned in the [remote attach contract](#remote-attach-contract).
An unknown, spent, expired or foreign ticket is `not_found` with a null `id`
and closes the connection. Bridges do not occupy the 64 client slots.

The owner is the LfProcess holding the Session's terminal (`lf session serve`),
never the provider's AgentProcess beneath it. The server resolves the Session's
current owner when it issues the ticket, without starting or resuming a provider.
No running owner returns `not_found {session}`; a missing owner endpoint at
issue or redemption returns `not_found {owner_lf_process_id}`.
When the owner ends, the bridge closes.
`lf session resume` and `server::client::attach` perform both ticket steps.

Each terminal has its own connection, so readings never wait behind terminal
bytes and a control failure never closes a bridged terminal. Follow
[terminal and provider recovery](#terminal-and-provider-recovery) to reconnect.

## Events alongside readings

Watch a Task to keep its current state visible and react when something happens.
After the `hello`/`welcome` exchange above, subscribe with `resume: null`:

```text
→ {"type":"subscribe","id":"task","operation":{"kind":"task.get","directory":"/repo","issue":"LOO-1"},"select":null,"resume":null}
← {"type":"response","id":"task","trace_id":"…","result":null}
← {"type":"reading","id":"task","checkpoint":"opaque-reading-token","frame":{"answers":null,"unavailable":null,"body":{…}}}
```

The reading supplies the current Task. An `event {id,seq,event}` on that
subscription describes an occurrence with a typed payload: a Task shipped, a
steer arrived, or a Graph step finished. A board can animate the ship; a Task
thread can append the arriving entry from its payload without requesting the
whole reading again. A reading remains a complete state replacement, and an
event is neither a replacement reading nor a patch to one. Loading an already
shipped Task does not mean it just shipped.

`task.get`, `task.wait`, `plan.list` and `desktop.view.list` deliver events for
Tasks in their complete reading, even when `select` hides those fields. A Task
leaving a plan still delivers its final observed occurrences; a newly included
Task starts with a baseline. Other subscription kinds currently carry readings
only.

| `event.kind` | Payload |
| --- | --- |
| `task.shipped` | `task_id`, `pr_id`, `number`, `url`, `merge_commit`: a recorded PR merge |
| `task.steer` | `task_id`, `steer {id,sequence,source,revision,author,text}`, `comment`: immutable direction and the original `TaskComment` when present (`null` for other direction) |
| `graph.step_finished` | `task_id`, `graph_process_id`, `step`: the same `GraphStepProcess` as a Task reading, including its recorded outcome |

`task.shipped` means a PR merge was recorded, not that Auto-Ship was enabled or
the Task was completed. `graph.step_finished` includes failed and interrupted
steps; inspect `step.outcome` before presenting a successful finish.

```json
{"type":"event","id":"task","seq":1,"event":{"kind":"task.steer","task_id":"task_11111111111111111111111111111111","steer":{"id":"11111111-1111-1111-1111-111111111111","sequence":7,"source":"comment-7","revision":"1","author":{"kind":"user"},"text":"Keep the examples."},"comment":null}}
```

A changed reading precedes the occurrences observed with it. Use the PR, steer
or step identity to avoid appending content already present in that reading.
For a comment steer, append `comment` by its own ID; its body is the original
thread text, while `steer.text` includes the context sent to providers.
Overlapping subscriptions can deliver the same occurrence with different
`seq` values. Deduplicate by occurrence identity if they feed the same view;
the delivery sequence is not an occurrence ID.
Events are never coalesced. A subscription that exceeds the bounded pending
batch ends with an error; reopen it for a fresh reading.

`id` identifies the subscription. `seq` orders event frames across the whole
connection, starting at 1 and increasing once per delivered event, including
events for other subscriptions; track it before routing
frames by `id`. Readings, confirmation checkpoints and opaque reading tokens
have separate meanings. Never use an event sequence as a reading checkpoint or
compare it across connections. A single subscription's sequence numbers need
not be consecutive because other subscriptions share the connection.

Keep the last complete reading as the recovery baseline. If event delivery has
a gap, stop applying incremental event updates and request a fresh reading for
every event-consuming subscription on that connection. After a disconnect,
subscribe with `resume: null` to obtain a complete current reading before
continuing event-driven updates.
Replace state from that reading without replaying animations or appending its
entries again. There is no event replay cursor: events missed while disconnected
are recovered as current state, not delivered as historical notifications.

Clients that only display readings can continue using reading checkpoint
resumption below. A checkpoint can suppress an unchanged reading; it cannot
establish that no events were missed. Event frames belong to the subscription's
lifetime and stop when it ends or is cancelled. Unknown event kinds must not
break unrelated subscriptions; recover the affected state with a fresh reading
if the client cannot apply an event.

Swift's `ServerClient.subscribeEvents` delivers readings and typed occurrences
in one ordered stream and reconnects event consumers with `resume: null`. A gap
ends all event-consuming streams on that connection; an unknown kind ends only
its event-consuming subscription. Reopen the ended stream to get current state.
Reading-only consumers ignore event frames and retain checkpoint resumption.

## Read a screen's data

```sh
lf plan board --json             # compact Task rows for a board
lf task detail LOO-1 --json       # the selected Task and its supporting detail
lf session card session_123 --json # a recorded Session's current card
```

Run these in the repository's checkout, replacing the Task and Session IDs
with ones from your Work. Each command prints the same JSON value its API
operation returns in `response.result`.

| Read | Contents |
| --- | --- |
| `plan.board` | `tasks` evidence with each Task's `id`, `identifier`, `title`, `section`, `state`, `pr.url` and nullable `waiting_on` actor/reason; `generated_at` |
| `task.detail` | Task, PR, checks, Graph processes, comments and current Session |
| `session.card` | Session, liveness, control holder and last line |

Use `section` to place a Task in Backlog, Working, Autonomous, Done or
Abandoned; `state` describes its progress within that grouping. To populate a
board row's detail, request `task.detail`; for its Session, request `session.card`.
A card observes the Session: reading it does not resume a provider, attach a
terminal or claim input control. Its `liveness` is `live`, `dead` or `unknown`;
`control_holder` is the recorded occupied terminal ID, never an input token.
`last_line` is the final nonempty line of the current provider preview, or null
when there is none; the Session's preview retains its source and observation time.

These are one-shot reads. `plan board` takes the same optional Wave, `--task`
and `--all` filters as `plan list`. Unreadable planning remains unavailable
rather than an empty board. `waiting_on` is null outside waiting or blocked
conditions. `task.detail` includes the planning `task`, derived `state` and
`section`, nullable `pr`, `checks`, `comments` and `current_session`, plus
`graph_processes`. Checks are the recorded required-check observation, pinned
by `head_sha` and `observed_at`, with aggregate state and named failing checks;
null means checks have not been observed. `current_session` is the Task's
recorded primary conversation, not whichever run started most recently.

Each read has its own Rust DTO mirrored in Swift. Typed clients send
`select: null` and decode the read's complete DTO, with no field defaults.
Use the host's `lf server describe --json` for operation arguments and the
`x-result` schema. Missing optional data, such as a Task with no PR or current
Session, is represented by the DTO's optional fields; do not manufacture a PR,
Session or live owner.

`select` remains useful for scripts and agents that consume arbitrary JSON.
A selected `plan.list` response cannot be decoded as the board DTO, and
selecting fields out of any typed response can make it undecodable. Request
the screen read when a typed client needs a smaller payload. Desktop's
outline continues to use `plan.list` for its richer runtime data.

Handle failures through the [protocol error codes](#local-protocol-revision-4).
Check the named Task or Session when it cannot be found. If the host cannot
answer, retain any previous display as stale and retry the read after recovery;
an unavailable Session card does not establish that the Session ended.

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
keeps its stream and queued frames and reconnects. Reading-only watches resume
from that reading's token; event consumers request a fresh baseline. A complete reading replaces it when the token is unknown. The newest
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

View a Session's running terminal from a phone: request a ticket on the control
connection, then redeem it on a separate bridge from the same device and account.
This planned contract requires LOO-718's authenticated remote transport.

Both WebSocket endpoints authenticate before upgrade and admit only the host
owner's Tailscale user; a rejected connection cannot present a ticket. The ticket
binds the two connections to the same Tailscale node stable ID and user ID, so
another device on the same account cannot redeem it. Neither a ticket,
client-supplied identity nor a proxy's Unix UID substitutes for authentication.
Local and remote tickets are not interchangeable.

Connect to the host's `/v1/machine` WebSocket, complete
[`hello`/`welcome`](#local-protocol-revision-4), and request `session.attach`
with a Session ID from `session.list`. Owner resolution, bridge protocol and errors follow
[terminal attachment](#terminal-attachment). A null `owner_lf_process_id`
selects the current terminal owner:

```text
→ {"type":"request","id":"attach-1","operation":{"kind":"session.attach","session":"<session ID>","owner_lf_process_id":null},"select":null}
← {"type":"response","id":"attach-1","trace_id":"<trace ID>","result":{"ticket":"<ticket>","owner_lf_process_id":"<owner ID>","expires_in_ms":60000}}
```

From the same authenticated device and account, open a separate WebSocket to
the host's `/v1/ticket` endpoint and send `attach` first, without `hello`:

```text
→ {"type":"attach","ticket":"<ticket>"}
← {"type":"bridged","owner_lf_process_id":"<owner ID>"}
```

Keep `/v1/machine` for requests and live Work updates.
For reconnection and exact-owner attachment, follow
[terminal and provider recovery](#terminal-and-provider-recovery).

Each ticket opens one bridge and expires 60 seconds after issue, including host
sleep; expiry does not close an open bridge. Every admitted terminal redemption
attempt consumes the ticket, including identity and local/remote mismatches.
After a ticket failure, retry with a fresh ticket and bridge.

## Subscription cost experiment

`scripts/benchmarks/subscription_baseline.py` drives an isolated test server
through the production connection handler. Copy the optimized library test
executable out of `target/` before comparing builds:

```sh
cargo test --release -p loopflow --lib subscription_ --no-run
uv run python scripts/benchmarks/subscription_baseline.py /tmp/frozen-subscription-server --output /tmp/subscription-samples
```

The runner uses 1/16/64 unpaged Session subscriptions, multiplexed and physical
connections, real checkpoints, ten-second quiet/active windows, 32/128 single
cycles and five cohort cycles. A separate writer records capture facts and
attachment-valid activity. Server CPU excludes that writer and the driver;
writer CPU and reaped-child CPU are separate. `--profile` produces a separate
macOS stack sample; exclude that run from timing comparisons. The fixture pins
two Tokio async workers per runtime and four server blocking workers to bound
resource creation. This does not measure installed defaults, Task resolution,
Process projections or kernel wakeups.

Counters are test-only and opt-in. SQL tracing resets VM status after each
execution so cached statements are counted once. Caller-thread CPU for runtime
construction/destruction excludes work on the runtime's worker threads; whole
phase CPU includes all server threads. Identical server demand shares completed
projections; distinct demand remains independent. Cancellation and saturation
regressions exercise production subscription creation and SQLite pulls with
deterministic pauses. Frozen pre-sharing measurements retain their original
per-consumer work counts and cancellation failure; candidate results are separate.

`--default-config --counts 1,16 --multiplexed-only --rounds 1` is a separate
bounded pilot with the ordinary runtime defaults. It removes
`TOKIO_WORKER_THREADS` and the fixture server's worker/blocking caps; it never
runs a 64-subscriber cohort. Keep its results separate from the capped matrix.
Resource inventories run outside timing phases. Their `ps` child CPU appears
separately in resource readings; cumulative child totals also include setup,
so use phase deltas when discussing application child work.
The pilot also measures 32 sequential runtime-only create/drop cycles, with no
SQL or subscribers and at most one extra runtime alive. That phase's process
CPU includes its workers through shutdown; it is separate from reader counters
and complete subscription cycles.

## Client contract

Submit a Task, lose the connection before its answer, then restart the client.
Recover by looking up the saved creation request on its original destination.
The receipt identifies the original Task; opening new subscriptions refreshes
its Work; attaching to the exact terminal owner recovers its display. These are
separate operations. None grants permission to repeat an uncertain launch.

Desktop, CLI and other Work clients use the shared operation kinds and result
types. Desktop sends Task creation, run, ship and Machine moves, Wave Machine
changes, rename and removal, and process history through this protocol. Client
code must not substitute CLI stdout parsing for a shared Work operation.
Ordinary CLI use can call the shared Rust functions without a running server.
Remote authentication and pairing are separate from this same-user socket.

### Cache and reconnect

Negotiate a revision on each new connection and use that revision's schema.
Keep `welcome.instance` and limits for that connection; an instance change
means the server restarted. Transport request IDs, subscription IDs and numeric
confirmation checkpoints belong to one connection. Opaque reading checkpoints
can resume a watch within the same server incarnation.

Last Work readings may remain visible as stale data. Resubscribe to every needed
read with its retained reading and opaque checkpoint, as described in
[Reconnect a watch](#reconnect-a-watch). An unchanged reading needs no replacement;
a changed reading or invalid checkpoint yields a complete current reading.
For subscriptions that consume typed events, follow
[event recovery](#events-alongside-readings) and request a fresh baseline after a
gap or reconnect. There is no missed-event replay or universal event log.
Coalescing replaces only complete
readings of the same subscription; checkpoint answers, errors and the final
reading before `ended` retain their ordering.

| Failure | Client recovery |
|---|---|
| Connection lost | Reconnect with bounded backoff, renegotiate and resubscribe. Recover creations by receipt; inspect other uncertain writes. |
| Request admission `busy` | Delay that operation; it was not dispatched. Keep the healthy connection. |
| Checkpoint overflow `busy` | Open a replacement subscription; the old subscription ended. Keep the connection. |
| Connection-level `busy` | Reconnect after a delay; no operation was admitted on that connection. |
| `incompatible` | Update the side named by `data.update`. |
| `invalid`, `not_found`, `conflict` | Correct the arguments, target or conflicting state; reconnecting alone cannot fix it. |
| `unavailable` | Retry the affected read when its dependency returns. A failed receipt lookup is not absence. |
| `outcome_unknown` | Look up the creation receipt, or inspect the resource for other writes. Never replay an arbitrary mutation. |

Desktop retries request-admission refusals (`busy` with `limit` and `max`) for
up to 60 seconds, keeping the connection. Backoff starts at 100 ms and caps at
1600 ms so initial subscription readings can drain before another attempt. It does not retry oversized
answers, disconnected commands or unknown outcomes. Reads keep their overall
deadline; cancellation also stops the backoff.

### Admission and retained work

`welcome.limits.max_open` bounds open requests plus subscriptions;
`max_retained_bytes` bounds per-connection producer reservations and serialized
answers through socket delivery. Reserve before dispatch or subscription
materialization, and include pending producers. A process-wide budget also
bounds retention across clients. This accounting is not a process-RSS limit.
Clients use advertised limits rather than hardcoding server defaults.

The initial limits are 256 open requests/subscriptions, 16 concurrent writes,
64 MiB retained per connection and 256 MiB process-wide. A producer reserves
one maximum-size message (32 MiB) before work starts, then holds the serialized
answer through delivery. These provisional limits bound retained output, not
the size of the store or total process memory.

Shared readers reserve from that same process budget before constructing a
reading. Their retained latest values remain charged until the last reader or
delivery releases them, with a 128 MiB aggregate ceiling so refreshes and output
retain headroom. Each consumer reserves connection and process capacity before
cloning and encoding its frame. Reconnect tokens suppress unchanged initial
delivery independently for each consumer; they never suppress an explicit
checkpoint response.

Writes have a separate admission cap. A refused write has not run; an admitted
write finishes even if its answer can no longer be delivered. Backpressure
cannot turn an admitted effect into an unrun `busy` refusal. Subscriptions wait
for room and re-read current state, retaining checkpoint obligations. Bounded
control headroom keeps refusals and acknowledgments deliverable; a peer that
does not drain still reaches the five-second send deadline.

### Creation receipts

`task.create` and `session.new` require a client-chosen `request_id` of 1–128
bytes. This is the durable creation identity, distinct from the outer request's
connection-local `id`. Use a fresh identity per submission and retain the exact
payload and destination for recovery. `lf task create` generates one unless
`--request-id` is supplied.

Before provisioning, the shared creation operation records the request identity,
content fingerprint and reserved resource identities. Concurrent retries and
server restarts reuse those identities. The same ID with different operation
content returns `conflict` without running that content. Receipts are retained
without age-based pruning; an old ID never becomes a new creation because time
passed or its resource was removed.

Task creation returns `{task, graph, session_id, milestone}` with or without
`run`. Milestones distinguish filing, admission and confirmed start. If a launch
was requested but its registration cannot be confirmed, the result is
`outcome_unknown` with the Task and `run_requested` milestone. Only a launch
correlated to this creation can settle that outcome; another current Graph is
insufficient. Recovery never launches again across that uncertainty.

`session.new` records its stop and new-thread admission against the same request.
A completed retry returns the current record for the same Session without
stopping anything again. An interrupted stop or input append must be reconciled
against that attempt; it cannot stop a newer process or append duplicate input.

`creation.get` is read-only. It returns the receipt with a derived `disposition`
and human-readable `explanation`, or `null` for confirmed absence. `complete`
means present the result; `continue` means the identical submitted request can
finish local preparation or proceed to its first launch; `inspect` means retain
it without resubmitting. The creation handler checks again under its request
lock, so a lookup cannot authorize repeating an uncertain effect. The receipt
retains original `run` intent; disposition is computed from the receipt and
current resource, never persisted as another outcome.

After reconnect or cold startup, look up the receipt first. Send the saved bytes
only for confirmed absence or `continue`. Unavailable lookup stays unavailable.
An interrupted launch says “Task created; start unconfirmed”; an interrupted
Session stop says “Conversation change unconfirmed”. Removed resources remain
identifiable through receipts and cannot be recreated. Newer Session input
prevents continuation of an unpublished older change.

Desktop persists only `{request_id, destination, payload, submitted_at}` before
first send, with a versioned record and exact payload. The destination pins the
original Machine and repository. Server receipts own outcomes; client records
contain no milestone ledger. A single app recovery owner serves all windows.
Storage failure keeps the form and reports an error before sending. A mounted
Task or Session result retires its submission, including Session promotion;
a socket reply or navigation request alone does not. Cold recovery offers
Show result and retires the request after displaying the recovered resource.
Failed removal repeats lookup and presentation safely. Inspect retains unresolved
outcomes and exposes the raw receipt. This is neither an editable
outbox nor offline creation, cross-device submission sync or generic replay.

### Terminal and provider recovery

Terminal continuation addresses the exact owner, separately from Work reads.
The owner alone grants input and geometry and revokes stale control.
Byte-stream viewers use Resume with a bounded output spool of 1 MiB and a 60-second
reconnect grace; expired continuation falls back to a fresh screen from that
owner. Terminal-mode viewers request a fresh screen. Uncertain input is
never replayed. Recovery takes only free claims or those held by the viewer's
own stale connection; a newer viewer keeps input and geometry. Replacing that
stale socket cannot let its later close revoke the replacement's claims.
A stalled bridge cannot block another viewer or Work control traffic.

Pass `owner_lf_process_id` to `session.attach` to reconnect only to that owner.
If the Session has a new owner, attachment returns `not_found` with
`owner_lf_process_id` and `current_lf_process_id`. Passing null reaches the
Session's current owner; neither attachment starts or resumes a provider.
A lost connection alone does not establish process death.

Desktop reconnects to the exact owner with bounded backoff. See
[connection loss](../../../../docs/server.md#keep-editing-through-a-connection-loss)
for connection status and recovery controls outside terminal bytes.

The Loopflow Session, provider conversation, serving AgentProcess and viewer's
control claims are distinct. `SessionRecord.transition` reports the latest
observed provider change, or null when none is known; `session.history` keeps
the changes. Reserving a process is not a successful transition.

| Transition | Meaning |
|---|---|
| `retained` | The same AgentProcess continues serving. Attachment may report this without replacing the latest provider-change event. |
| `resumed` | A replacement AgentProcess serves the same provider conversation. |
| `forked` | A new provider conversation carries source history, seeded natively or from the log. |
| `started` | A new provider conversation starts without the earlier history. If a source is recorded, that conversation was left behind. |

Transitions identify source and destination provider conversations and processes
when known. Missing lineage remains unknown. A fork's `original_running`
describes liveness at the transition; current liveness requires a fresh reading.
Desktop shows the short explanation beside the terminal and keeps the detail in
history. Reading either grants no authority to drive the provider.
