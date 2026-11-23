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
stop the service. Work sessions and Desktop shells run in a holder the service starts with
`setsid` and controls ([Held terminals](../../../loopflow-execution/src/transport/README.md#held-terminals));
stopping or replacing the service leaves holders and their agents running.
This service holds no PTY itself and hosts no Graphs: those run as
[plain detached processes](../../../loopflow-execution/src/transport/README.md#headless-work). Logout/reboot do not
preserve live processes; the [recovery pass](#recovery-pass) starts them again.
Disabled Background Items remain disabled. Upgrade
retires the previous
`com.loopflow.machine` registration before starting `com.loopflow.server`.
The old bundled plist remains only for SMAppService approval inspection/removal.
Development jobs and Linux units migrate their hashed labels the same way;
existing disabled/masked units require explicit enablement. The `machine.sock`
address and its lock remain stable so already-running relays can recover across
the rename. There is one listener, with no alias or alternate protocol path.
Work and credentials stay in place.

For a repository without Linear or Asana the service also reads the shared
plan. A save, here or in the CLI, sends its own records at once
(`plan_carriers::send`: the hub, and a Git push with no fetch, five seconds at
most). The service reads and sends (`plan_carriers::sync`) after each command
it answers that names a directory, and fetches every such repository's plan
refs every 60 s, which is when another Machine's change arrives with no hub.
[Without a tracker](../../../../docs/dev/planning.md#without-a-tracker-stamps-and-carriers) has the carriers.

## Recovery pass

```sh
kill -9 <pid of a Task conversation's claude>
lf task status LOO-12   # Conversation: running (started again 12:01, attempt 1)
```

A watcher thread (`recovery.rs`) reads the store when its planning, Session or
process revisions move, when a conversation's or a run's backoff comes due,
and at least once a minute. The server writes nothing on the way down; every fact
the pass needs is written before it is needed (`meta.json`, the holder's
`exit.json`, Session and Graph-run rows with pid and OS start time). Its
first reading after start is the restart and reboot case. Each reading:

1. **Settles**, at most every 10 seconds or when a run's backoff came due: reaps orphan agents, releases
   AgentProcess ancestry whose parent was never recorded, carries Task runs
   (step 3), continues limit waits and checks rentals
   (`ops::recovery::settle`). Reads never do this.
2. **Starts Task conversations.** A Task's primary Session starts again when
   the Task is open, claimed and checked out here, no agent or pending holder
   serves it, and it never had an agent or its last agent did not end by the
   person (exit 0 or 130).
   An unfinished agent whose recorded process is gone died too. The store
   counts the start (`agent_sessions.starts_since_turn`, `last_started_at`)
   before it goes through `session.continue`'s path with request
   `recover:<session>:<ended agent, or new>:<attempt>`, so one death starts one
   provider across server restarts. `lf task run`, `lf task auto on` and
   `lf task checkout` only prepare the Session; this pass starts it. That
   first start counts only if it fails, so the first death after it comes
   back at once; its terminal holds later passes off. The pass's
   `lf session resume` carries that request as `--request-id`; a resume with
   any other request, or none, is a person's, and sets the count to zero and
   clears `start_failure`.
3. **Carries Task runs.** A Task whose `lf task run` carrier is dead gets
   `lf task run <ID>` again, from its checkpoint; with auto on it stands
   in at interactive steps by itself. A carrier that exits on its own clears
   its OS process; one found dead died, and retention, which clears it too,
   records when (`task_graphs.run_died_at`). The pass counts those starts
   (`task_graphs.restarts`, `restarted_at`), reset when the run finishes a
   step, and waits for a person after five.

Store retention runs beside the pass, on the maintenance thread
(`maintenance.rs`): at start and every hour it ends owners whose OS process
is gone.

One start is in flight at a time: the next begins when the last one's
agent attaches, or after 3 seconds, most recently updated Task first. Sessions
and carriers share one backoff: the first start after a death is immediate,
later ones wait 1 minute doubling to 30, and after 5 starts without a finished
turn or step the Session and the Task run show the command that continues. A
start that fails records its reason on the Session
(`agent_sessions.start_failure`, for example `claude is not installed on this
Machine's PATH`), cleared by the next start that succeeds, and retries on the
same backoff. A first start that succeeds after failed ones clears their
count too: nothing started again. A start under way (the agent is attached to
its open held terminal and has no pid yet, `SessionSummary.launching`) reads
`starting`, not `unknown`. A Session's `ended {at, exit_code}` and `start {attempts,
last_started_at}` carry these facts, and its `outlook {kind, at, reason,
command}` says once what happens next: `starting`, `coming_back`,
`waiting_for_you`, `quit` or `could_not_start`. Whether the server starts a
conversation is one rule, `ops::recovery::server_start`: quit, the give-up,
whether the Task is open, checked out and claimed here, whether a server
runs, then the backoff. The pass asks it of each open Task's conversation
here (`task_conversations_here`) and the Session read
(`ops::human_session::session_ending`) asks it for the outlook, so a line
never promises a start the pass will not make. That read also writes the
sentence, `status_line`; `lf task status` and Desktop print it and derive
nothing.
A holder that gets no PTY answers `no PTY left: N in use (<limit>)`, read the
way `lf machine doctor`'s `ptys` check reads it. Only a Task's work session is
started; a Wave's or repository's work session, a chat and a batch are
reported, never started.

## Local protocol, revision 9

One connection per client over the same-user Unix socket. Every message is one
JSON object per line (`serde_json` never emits a raw newline). Control messages
are capped at 1 MiB and connections at 64; a connection has five seconds to send
its first message and five seconds to take any message the server sends. Larger
messages and terminal bytes travel on ticketed connections (below). The socket
and its directory are private; remote proxies must never forward this endpoint.
The remote carrier (`remote.rs`, `lf server mobile`) speaks the same protocol
over WebSocket on the host's Tailscale address: one text frame per message on
`/v1/machine`, ticketed connections on `/v1/ticket`, binary frames for terminal
bytes. It admits another untagged node of the approved owner through
`tailscale whois`. The Machine's own node is refused unless
`mobile-same-device` exists in the lf home, which is how an iOS Simulator on the
same Mac connects; create it only on a Mac with one account. Setup saves the endpoint, owner and enabled choice in
Machine-local `mobile.yaml`, using the shared atomic file writer. It does not
add service state to `config.yaml` or the preferences catalogue.

Another Machine reaches this server over SSH: `lf server connect` checks
`LF_EXPECTED_MACHINE_ID`, starts the server when none is running
(`installation::server::ensure`), and copies bytes between its stdin and stdout
and the socket until either side closes. It reads none of them, so every
message works over SSH as on the socket, and it records nothing; the server
records each request. An SSH login is the same OS user the socket trusts. An
`lf` older than this entry exits 2 with a usage error, which the sender
reports as the destination's version and its update command; an older server
answers `incompatible` itself. The client side is `Connection::open_on`.

Another Machine says so in `hello` (`client.kind` is `machine`), and every
operation it sends then runs here and is never sent on. Its requests and
subscriptions carry `sender`, what the receiving server cannot know; a
Machine's null `sender` is `invalid`, and a client on this Machine sends null:

```text
→ {"type":"hello","revision":9,"min_revision":8,"client":{"kind":"machine",…}}
← {"type":"welcome",…}
→ {"type":"request","id":"…","operation":{"kind":"task.auto.off","directory":"/laptop/repo",…},"select":null,"sender":{"repository":"repo_…","selection":{"profile":{…},"participant":"…","agent":null,"settings":null,"source":null},"secrets":{}}}
← {"type":"response","id":"…","result":{…}}
```

The server runs the operation in its own checkout of `repository` (the
operation's `directory` is the sender's), and starts any launch (`task.run`,
`task.auto.on`) with `selection` and `secrets`: the values `forward.env` names,
which nothing prints (`Debug` shows names). Files `forward.files` names are
delivered over SSH first. The sender waits 120 s for an answer; past that the
request fails, naming the Machine and `ssh <route> lf server status`.

`machine/wire.rs` owns the envelope, endpoint and native request types;
`server/wire.rs` instantiates the envelope with the CLI read/write operations.
The Machine client and runtime depend on those values without importing dispatch; `swift/Loopflow/Services/ServerProtocol.swift`
mirrors them. Executable fixtures live in `rust/loopflow-execution/tests/fixtures/dto/server/`.
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
For example, `task.auto.on` returns `TaskRun`: its `state` is `running`,
`waiting` or `ended`, with the fields described by that variant.
`lf task auto on <issue> --json` prints the same result. Use the operation's
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
→ {"type":"hello","revision":9,"min_revision":8,"client":{"kind":"desktop","platform":"macos","name":"Loopflow","version":"0.15.1"}}
← {"type":"welcome","client_id":"…","revision":9,"server_revision":9,"min_revision":8,"instance":"…","build":"0.15.1","executable_sha256":"…","machine":{…},"limits":{"max_message":1048576,"max_connections":64,"max_open":256,"max_retained_bytes":67108864}}
→ {"type":"request","id":"a","operation":{"kind":"plan.list","directory":"/","wave":null,"task":null,"all":true},"select":null,"request_id":null,"sender":null}
→ {"type":"subscribe","id":"b","operation":{"kind":"session.list","directory":"/repo","headless":false,"after":null,"limit":null,"task":null},"select":["id","title","state"],"resume":null,"sender":null}
← {"type":"response","id":"a","trace_id":"…","result":{…}}
← {"type":"response","id":"b","trace_id":"…","result":null}
← {"type":"reading","id":"b","checkpoint":"opaque-reading-token","frame":{"answers":null,"unavailable":null,"body":{…}}}
→ {"type":"checkpoint","id":"b","checkpoint":3}
← {"type":"reading","id":"b","checkpoint":"opaque-reading-token","frame":{"answers":3,…}}
→ {"type":"cancel","id":"b"}
→ {"type":"request","id":"c","operation":{"kind":"task.run","directory":"/repo","issue":"LOO-1","graph":null,"from":null,"agent":null,"approve":false},"select":null,"request_id":null,"sender":null}
← {"type":"response","id":"c","trace_id":"…","result":{"state":"running","issue":"LOO-1","run":"code from start","merges":false,"graph_process":"…","log":"…"}}
→ {"type":"subscribe","id":"d","operation":{"kind":"task.wait","issue":"LOO-1","until":"submitted"},"select":null,"resume":null,"sender":null}
← {"type":"reading","id":"d","checkpoint":"another-token","frame":{"answers":null,"unavailable":null,"body":{"reached":true,"task":{…}}}}
← {"type":"ended","id":"d"}
```

The first message is `hello` with the revisions the client speaks. The server
chooses the newest revision both sides speak and answers `welcome` with it, a
server-assigned `client_id`, the server's own range, its incarnation (`instance` changes with every start), build,
executable SHA-256, the Machine reading (`null` while the registry is
unavailable or uninitialized) and limits. The negotiated revision is the whole
compatibility check: a server that speaks it offers every operation below, so
no capability list travels. With no revision in
common it answers `error {code: incompatible}` naming the side to update in
its message and in `data.update` (`client` or `server`), then closes. This build speaks revisions 8 and 9; the shape lets a later server speak
two. A revision-1 client's four-byte length prefix starts below `0x20`; the
server answers it a revision-1 framed `incompatible` error carrying `instance`
and closes. That sniff is the whole compatibility layer and goes with revision 1.

Revision 7 answers each write once it has committed, with `revisions`: the
store domains it advanced and their value after the commit. Planning sync and
a claim's Linear assignee run after the answer. Each `reading` frame carries
`read_at`, the revisions of the snapshot it read, so a client holding a change
on screen drops it at the first reading at or past the write's `revisions`. A
write may carry `request_id`; for a day a repeat under that ID gets the first
answer and runs nothing, and a different operation under it is `invalid`. An
`error`'s `revisions`, when not null, are the domains that advanced while the
write ran: other writes, such as an earlier write's planning sync, land in the
same window, so they do not say the failed write took effect. Older clients
must update.

```text
→ {"type":"request","id":"c-1","operation":{"kind":"task.complete","directory":"/repo","issue":"LOO-2"},"select":null,"request_id":"6f1c…"}
← {"type":"response","id":"c-1","trace_id":"…","result":{…},"revisions":{"planning":412}}
← {"type":"reading","id":"tasks","checkpoint":"…","frame":{"answers":null,"unavailable":null,"body":[…],"work":null,"read_at":{"planning":412,"sessions":90,…}}}
```

Revision 6 adds per-request SQL receipts (`performance.read`, `performance.write`) and subscription
SQL evaluation IDs.

Revision 5 combines bulk/upload tickets, mobile access, shared subscription
readers and typed events. Control messages are limited to 1 MiB; ticketed
messages retain the shared engine's 32 MiB response limit and 64 MiB per-client
budget, leaving headroom for other reads during one large transfer. Older
clients must update. Install the matching Desktop/CLI/server together and
update the phone app before connecting.

`status` before `hello` is the readiness probe: `status {revision, min_revision,
instance, executable_sha256, clients}`, then the server closes. `lf server status` and
installation use it so readiness never opens Work or speaks the full protocol;
`lf server status` also reports `listening` (a server accepted the connection),
`revision` and `clients`: each welcomed connection still open, oldest first, as
`{id, client, connected_at, requests, open, retained_bytes}`, where `client` is the identity its
`hello` gave and `requests` counts its requests, subscriptions included. The
`open` and `retained_bytes` fields report current admitted work and retained-byte
accounting, including pending producers. The
identity is for diagnostics and grants nothing. `ready` means the server answers at a revision the asking build speaks; a
different executable is not an error, so a branch build reports the installed
server ready. Installation alone compares `executable_sha256` with the selected
installation, to decide whether to register again.

After `welcome`, many requests share the connection. A `request` carries an `id`
(1–128 bytes, unique among the connection's unanswered requests) and an
`operation`; the server answers `response {id, trace_id, result}` or `error
{id, code, message, data}` in whatever order work finishes. `result` is the
operation's value: what its CLI spelling prints with `--json`, a ticket
`{ticket, terminal_id, expires_in_ms}` for `session.attach`, and null
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
| `not_found` | The named terminal, subscription, repository checkout, view member or record does not exist | what was named: `{terminal_id, current_terminal_id}`, `{id}`, `{repo_id}`, `{expression}` |
| `conflict` | The request contradicts current state: a still-unanswered transport ID or a creation ID reused with different content | `{id}` or `{request_id}` |
| `unavailable` | A dependency is missing or failed: the Machine registry, a subscription, a read | `{resource}` when one is named |
| `outcome_unknown` | The operation may have run; inspect its creation receipt when available, otherwise the resource | creation details when known, otherwise null |
| `busy` | Admission capacity reached, an oversized read answer, or retained subscription capacity is exhausted | `{max_connections}` at connection capacity; `{limit, max}` for request admission; `{limit: "retained_bytes"}` for an answer that exceeds the message limit |

A shared operation names its code by returning `engine::Error` inside its
error; anything else a read returns is `unavailable`.

After editing a Task's checkout, request its changed files on the same connection:

```json
{"type":"request","id":"changes","operation":{"kind":"task.changes","issue":"LOO-1","base":"parent","head":null},"select":null,"request_id":null}
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

Resolved settings and the skill catalog are worked out once per change to the
files they came from, not once per reading. Save `.lf/config.yaml`, a personal
or global settings file, a Wave's files or a skill under `.lf/skills`,
`.claude`, `.agents` or `.codex`, and every subscription that read it gets a
new reading within about a second:

```bash
lf wave show mac --subscribe --json &
lf set default_agent codex    # a new line: Tasks with no agent of their own name codex
```

An edit that changes nothing in the selected answer sends no line. Until a
file changes, readings share one resolved value per checkout and Wave, one for
your global `config.yaml`, and one catalog per set of skill directories. A file that fails to load gives every
reading the same error it gives `lf config settings`, and the server warns
about it once per edit. A `--setting` override belongs to the command that
carries it: a read with one resolves for itself and leaves the shared value
alone. Settings from the environment are the server's own, the same for every
reading, and identify the shared value along with the checkout and Wave.

In code this is `observe::derived`: a value computed from observed facts,
kept with the facts its computation read and computed again when one changes.
`observe::Derived` lists every such value and what identifies it. A
computation reads observed facts and its key, nothing else; an input from the
environment, a thread-local or the store goes in the key. With no reading
active (any bare `lf` command) `derived` computes inline and keeps nothing.
A bare read that asks for one value many times, as Task details do for every
Task in a checkout, runs inside `observe::reading`: one reading that lasts for
that call.

Every operation names its `kind` as a dotted resource and verb, beside its
fields. `lf/commands/read.rs` (`Read`) and `lf/commands/write.rs` (`Write`) own
the shared kinds; `wire::ServerOperation` owns the server's own.

Rust variants use the kind's PascalCase spelling (`SessionList`); Swift cases
use lowerCamelCase (`sessionList`). Dots and underscores separate words. Each
case names one operation, including distinct claim/unassign and pin/unpin writes.

| API kind | CLI | Engine |
| --- | --- | --- |
| `task.get` | `lf task show <issue>` | read |
| `task.detail` | `lf task detail <issue>` | read |
| `task.list` (`closed: true`) | `lf task list --closed --wave <wave>` | read |
| `task.changes` | `lf task changes <issue>` | read |
| `task.diff`, `task.file`, `task.files` | `lf task diff`, `lf task file`, `lf task files` | read |
| `task.context` | `lf task context <issue>` | read |
| `task.runs` | `lf task runs <issue>` | read |
| `task.wait` | `lf task wait <issue> --until <state>` | `engine::task` |
| `task.create` | `lf task create [--session <id>] [--run] --request-id <id>` | write |
| `creation.get` | `lf server receipt <request-id> --json` | read; no creation or launch |
| `task.run` | `lf task run <issue>` | `engine::task` |
| `task.auto.on` | `lf task auto on <issue>` | `engine::task` |
| `task.auto.off` | `lf task auto off <issue>` | `engine::task` |
| `task.steer` | `lf steer <issue> <message>` | `engine::task`; the thread entry the store saves as the Task's steer |
| `task.report` | `lf task report <issue> <message>` | `engine::task` |
| `task.interrupt` | `lf task interrupt <issue>` | `engine::task` |
| `task.update` | `lf task update <issue>` | write |
| `task.move`, `.claim`, `.unassign`, `.save`, `.complete`, `.reopen`, `.abandon` | `lf task <verb>` | write |
| `session.list` | `lf session list` | read |
| `session.card` | `lf session card <id>` | read |
| `session.start` | start a held terminal, for example for `lf resume --task`, a Desktop pane or a Desktop Task shell | server |
| `session.attach` | Desktop panes' relay and other clients that speak only the server protocol; CLI views of a held terminal enter `held.sock` directly | server |
| `session.usage` | `lf session usage` (`lf usage`) | read |
| `session.history` | `lf session history` (`lf history`) | read |
| `session.retention` | `lf session retention` | read |
| `session.rename`, `.bind`, `.resume`, `.new` | `lf session <verb>`, `lf resume` | write |
| `session.steer` | `lf [--wave <wave>] steer [<id>] <message>` | write; `engine::session::steer` |
| `session.boundary` | none; provider hooks through `lf __context-block` ([Turn and tool boundaries](#turn-and-tool-boundaries)) | server |
| `plan.changes`, `plan.apply` | none; another Machine whose hub this is (`lf machine hub`), over `lf server connect` | server; stamped records in both directions, `ops::plan_hub::changes_here`, `apply_here` (revision 9); `plan.apply` answers how many records the hub could not write, which the sender keeps unsent |
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
| `repo.tracker` | `lf repo tracker` | read |
| `repo.tracker.answer` | `lf repo tracker answer <id>` | write |
| `repo.people`, `.tokens`, `.path` | `lf repo people`, `lf repo tokens`, `lf repo path` | read |
| `repo.release` | `lf repo release run [<version>] [-t <target>]`; the other `release` verbs are its steps, CLI only | write, detached; `engine::repo::release` |
| `repo.ci` | `lf repo ci [--since <since>] [--wave <wave>] [--repo <repo>]` | read; `engine::repo::ci` |
| `graph.list`, `graph.customize` | `lf graph list`, `lf graph customize` | read, write |
| `graph.run` | `lf graph <name>` (`lf -b`, detached; `task.run` keeps the Task spelling) | write; `engine::graph::run` (the Process carrying the Graph) |
| `graph.run.get` | `lf graph show <id>` | read |
| `session.history.get` | `lf session history show <session-id>` | read; rendered history |
| `machine.self`, `.list`, `.doctor` | `lf machine self`, `lf machine list`, `lf machine doctor` (`lf doctor`) | read |
| `work.resolve` | `lf open <url> --link`; a `work` noun would shadow the builtin `work` skill | read |
| `config.catalog`, `.settings`, `.get`, `.explain` | `lf config catalog`, `lf config settings`, `lf config get`, `lf config explain` | `engine::config` |
| `config.set`, `.unset` | `lf config set`, `lf config unset` (`lf set`, `lf unset`) | `engine::config` |
| `config.theme` | none; the Machine's `theme:`, read by Desktop and the phone | `engine::config` |
| `pr.publish`, `.open`, `.submit`, `.arm`, `.abandon`, `.reconcile`, `.request_review` | `lf pr publish`, `lf pr open`, `lf pr submit`, `lf pr arm`, `lf pr abandon`, `lf pr reconcile`, `lf pr request-review` | write; `engine::pr::publish`, `open`, `submit`, `arm`, `abandon`, `reconcile`, `request_review` |
| `pr.land` | `lf pr land` (`lf land`) | write; `engine::pr::land` (detached; the Process carrying the landing) |
| `pr.checks`, `.reviews` | `lf pr checks`, `lf pr reviews` | read; `engine::pr::checks` (no `--subscribe`), `reviews` |
| `checkout.commit`, `.sync`, `.create` | `lf commit`, `lf sync`, `lf wt create` | write; `engine::checkout::commit`, `sync` (detached unless `--plan`, `--continue`, `--abort`), `create` |
| `account.list` | `lf account [<provider>]` | read; `engine::account::list` (service authentication) |
| `account.connect`, `.disconnect` | `lf account connect`, `disconnect` | write; `engine::account::connect`, `disconnect` (service authentication) |
| `skill.run` | `lf skill <name>` (`lf -b`, detached) | write; `engine::skill::run` (the Process carrying the run) |
| `skill.list` | `lf list [<path>…]` | read; `lf::commands::list::list_children` |
| `desktop.portfolio`, `.portfolio.set` | `lf desktop portfolio` (`lf portfolio`) | read, write |
| `desktop.layout` | `lf desktop layout query:<id>` | read (Dashboard slots) |
| `desktop.view.list`, `.view.get` | `lf desktop view list`, `lf desktop view show` | read |
| `desktop.dashboard.*` | `lf desktop dashboard query`, `pin`, `unpin`, `rename`, `hide`, `show`, `move`, `resize`, `slots` | write |
| `desktop.<verb>` | `lf desktop enter`, `send`, `send-key`, `capture`, `zoom`, `focus`, `close`, `reopen`, `move`, `resize`, `browse`, … | CLI only, executed by the app |
| `view.select` | `lf desktop view list --all` | read |
| `session.continue` | the phone's Continue | held-terminal launch |
| `mobile.status`, `.configure` | `lf server mobile` | remote endpoint setup |
| `request.upload` | none | upload ticket for an oversized request |

"CLI handler; not served" is an operation only the CLI dispatcher runs today;
its kind fixes the name a served version keeps.

The nouns are Loopflow's primitives (Wave, Task, Graph, Session, Process,
Machine, Repository, Work, PR, Checkout, Skill, Account), plus `config` for
settings, `desktop` for the app and `plan` for every Wave's plan at once;
nothing gets a namespace of its own for a client.

`desktop.*` is Desktop's shared arrangement state: Dashboards. Pane layouts
stay in Desktop's per-Mac cache. The portfolio is the Machine's repository set: Desktop's sidebar,
the phone's `view.select` Task list and its creation destinations all read it.

- `config.*` is `engine::config`, which `lf config` calls too: `config.catalog`
  lists the typed settings, `config.settings {directory, wave, skill, graph,
  step}` resolves them with their sources, and `config.get`, `config.set
  {value}`, `config.unset` and `config.explain` take `{destination, key}`, where
  a destination names an owner (`personal_global`, `shared_repo {repo}`,
  …), never a file plus a scope. `config.theme {value}` reads or sets the
  serving Machine's `theme:`. A failed or uncertain save is never replayed; setting values are
  omitted from request telemetry.
- A read uses the existing `Read` shape and answers with the JSON the command
  prints, serialized once. A command that prints bare text
  (`config repository --path`) answers a JSON string. Every shared read is
  supported. Repository IDs resolve on the destination with `repo.path`;
  paths in local reads are destination paths. `view.get` returns the existing
  WorkViewPlan, including the recorded primary Session; roadmap membership alone
  cannot select one. A missing primary stays absent. Attach by that Session ID; the server resolves its held terminal.
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
- `subscribe` takes a read with a subscription: `desktop.view.list`, `view.select`,
  `session.list` without `limit`, `task.get`, `task.wait`, `session.history`,
  `plan.list` and `repo.tracker`. It answers
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
  `task.run {directory, issue, graph, from, agent, approve}` and `task.auto.on
  {directory, issue}` outlive the request, so the server starts their
  command (`lf [-b] [-a <agent>] task run …`, `lf task auto on <issue>`)
  as a detached process and answers what that process reports once it
  knows: `{state: running, merges, graph_process, log}` when the
  Graph registers, `waiting {step}` or `ended` when nothing ran, or the
  refusal it printed; `task.auto.on` answers the same after turning auto
  on. The connection never waits on the run. `approve` is `-b`: every step
  headless to `end`, writing no auto; without it
  the run stands in at interactive steps only while auto is on, and
  otherwise stops before the next one. A null `agent` uses each step's
  configured harness. `lf task run <issue> --json` and `lf task auto on --json`
  print the same value. `task.auto.off {directory, issue, reason}` turns auto off (`{issue, was_on}`); only `task.auto.on` turns it on, `task.steer` and `task.report {message}` save to the
  thread and return it, and `task.interrupt` returns its receipt.
- A write uses the shared `Write` shape. `Write::run` calls the shared Rust
  function its CLI handler calls and returns the typed value; the CLI prints
  that value with `--json` and renders it as text otherwise. Nothing parses
  standard output. `task.create {directory,request_id,title,request,wave,parent,graph,session,run}`
  files a Task or adopts the named Session; `run` admits its conversation,
  which makes the checkout and starts the Graph after the answer.
  A null `wave` selects the repository. Both paths return the same creation
  receipt, described below. A write whose Work
  lives on another Machine (`task.run`, `task.auto.on`, `task.auto.off`,
  `task.claim`, `task.unassign`, `session.resume`) goes to that Machine's
  server over SSH and returns its answer; it never falls back to
  local execution. Requests create no Process rows; diagnostics the shared
  functions print go to the request's log. A refusal the shared code names
  (an empty name is `invalid`, a stale Dashboard revision `conflict`, a missing
  record `not_found`) answers that code; any other failure is
  `outcome_unknown`, since the write may have taken partial effect. A lost
  response means outcome unknown; inspect state before retrying. Transport IDs
  correlate diagnostics and are **not** durable deduplication receipts. Only
  `task.create` and `session.new` have the creation retry contract below.
- `session.start {session, request, argv, cwd, env, rows, cols}` starts the
  Session's agent in this build's holder and answers `{terminal_id}`. `env` is
  the caller's complete environment; the server's own is not used. `request`
  identifies the start: repeating it answers the same terminal and starts
  nothing, and a start whose holder reply was lost is reconciled against the
  holder's inventory first; a start that could not be reconciled answers
  `outcome_unknown`, and repeating the same `request` settles it. It creates
  a process and, with `session.continue`, the only operation here that does;
  attaching never does.
- `session.continue {session, request, rows, cols}` starts `lf session resume`
  of exactly that Session in a held terminal, in its recorded directory, with
  the server's environment and its own `LF_HOME` and `LF_BIN`, for a client
  with no environment to send (the phone). It answers and repeats like
  `session.start`; a Session `run_refusal` refuses is `conflict`.
- Bulk. An answer or reading larger than `limits.max_message` never travels on
  the control connection: the client receives `bulk {id, ticket, bytes}`, redeems
  the ticket on a second connection, and reads `admitted` followed by the original
  message as one line. A request larger than the limit asks for
  `request.upload {bytes}`, receives `{ticket}`, and sends its one `request`
  line on the ticketed connection;
  the answer arrives on the control connection under the request's id. Ticketed
  connections carry up to 32 MiB and a transfer has two minutes; larger messages
  are refused with `unavailable`. Clients fetch spills off their reader so other
  request IDs never wait. Within one ID, the ticket keeps its place: deliver its
  payload before later readings, errors or `ended`, and advance the resume
  checkpoint only with the delivered reading. For that reason another
  admission's attempt leaves a bulk ticket redeemable: its payload cannot be
  requested again in its place, while a terminal ticket, which a fresh
  `session.attach` replaces, is spent by any attempt. Swift clients bound deferred
  messages to 64 frames and 32 MiB; falling behind requires reconnecting.
  A control line over the limit is answered
  `error {id: null, invalid}` whose data names `max_message`, and the connection
  closes. Nothing is paged.

## Terminal attachment

`session.attach` answers a ticket on an ordinary client connection. A separate
bridge sends `{"type":"attach","ticket":"<ticket>"}` as its only JSON message
and is answered `bridged`. A ticket (`ticket.rs`) is 16 random bytes as 22 base64url
characters and lasts 60 seconds by the wall clock, so host sleep counts; expiry
does not close an open bridge. A ticket binds to the admission that requested
it: the Machine's own user on the local socket, or one Tailscale node and user
remotely. Since a new ticket is one `session.attach` away, every redemption
attempt spends a terminal ticket, successful or not:
an unknown, spent, expired or foreign ticket is `not_found {ticket}` with a null
`id` and closes the connection. After a failure or a disconnect before
`bridged`, request a new ticket and bridge. Bridges read no Work and do not
occupy the 64 client slots.

`session.attach {session}` attaches to the Session's held terminal, and
`{session, terminal_id}` only to that terminal. Either answers a ticket
`{ticket, terminal_id, expires_in_ms}`, and its bridge answers
`bridged {terminal_id}`, then carries the
[view protocol](../transport/README.md#view-protocol): raw bytes locally,
binary frames remotely. A terminal that has ended answers `not_found` with
`terminal_id` and `current_terminal_id` (null when the Session has none).
A terminal whose holder has not answered this server since it started
answers `unavailable {resource: "holder", terminal_id, retry: true}`: its
agent keeps running, and the same request attaches once the holder answers.
A ticket presented after its terminal ended, or while its holder is not
answering, gets the same two errors with a null `id` in place of `bridged`.
Resolution never starts a provider; a Session an earlier build left under
its own owner process is restarted by `lf resume`, not attached
([transport README](../transport/README.md#conversations-an-earlier-build-started)).
`lf session resume` and `server::client::attach` perform both ticket steps.

Each terminal has its own connection, so readings never wait behind terminal
bytes and a control failure never closes a bridged terminal. Follow
[terminal and provider recovery](#terminal-and-provider-recovery) to reconnect.

### Remote attach contract

The [attachment contract above](#terminal-attachment) applies over the remote
carrier (`lf server mobile`), with these transport differences.

Connect to the host's `/v1/machine` WebSocket, complete
[`hello`/`welcome`](#local-protocol-revision-7), and request `session.attach`
with a Session ID from `session.list`:

```text
→ {"type":"request","id":"attach-1","operation":{"kind":"session.attach","session":"<session ID>","terminal_id":null},"select":null,"request_id":null}
← {"type":"response","id":"attach-1","trace_id":"<trace ID>","result":{"ticket":"<ticket>","terminal_id":"<terminal ID>","expires_in_ms":60000}}
```

Open a separate WebSocket to the host's `/v1/ticket` endpoint and send
`attach` first, without `hello`; `/v1/machine` keeps carrying requests and
live Work updates:

```text
→ {"type":"attach","ticket":"<ticket>"}
← {"type":"bridged","terminal_id":"<terminal ID>"}
```

Both WebSocket endpoints authenticate before upgrade and admit only the host
owner's Tailscale user; a rejected connection cannot present a ticket. Tickets
bind both connections to the same Tailscale node stable ID and user ID; another
device on the same account cannot redeem them. Client-supplied identity and a
proxy's Unix UID cannot substitute for authentication. Local and remote
tickets are not interchangeable. The 60-second lifetime includes host sleep.

### Terminal bridge endings

An orderly terminal ends its viewer stream with `0x07`, a four-byte big-endian
payload length and UTF-8 JSON, such as `{"reason":"replaced"}`. The server bridge
and raw relay forward it unchanged. It is outside PTY bytes and needs no event
nonce, so unfinished terminal escapes cannot swallow the reason.
`lf server describe --json` exposes the schema under `x-terminal-close`.

| Reason | Meaning | Client action |
| --- | --- | --- |
| `replaced` | This attachment yielded to another place or provider. | Request `session.attach` again for the same Session with `terminal_id: null`; retry with a bound while the next terminal becomes available. |
| `finished` | Execution finished. | Stop automatic attachment and keep the final screen. |
| `retired` | Task cleanup or shell Close ended this terminal. | Stop automatic attachment; deliberately resume when needed. |

Attachment finds existing execution. It never launches a provider or replays
keystrokes. A replacement terminal needs a fresh attach; every attached view
of it types, beside any already there. Desktop's
decoded relay shows the reason, disables input and stops recovery; it does not
automatically follow a replacement.

Shutdown allows a bounded drain. A slow or broken connection can miss the final
frame: EOF or timeout alone leaves the reason unknown. Server restart continues
to use the same held terminal and its PTY position, without ending the provider.
The existing heartbeat detects an unanswered return path independently of typing;
a quiet provider remains attached when the bridge answers. Missing execution is
reported, never started by an attachment attempt.

## Turn and tool boundaries

`Launch::kind` carries whether a launch is bare or managed and its mode.
[Work sessions, workers, chats and batches are defined here](../../../../docs/glossary.md#loopflows-words).
`transport::LaunchInput::kind` selects chats or batches for skill and Graph
commands launched in the caller's terminal or pipe. `lf -b resume <ID> --message TEXT`
is a batch. A Graph's correction turn in a Task run remains a worker.
Interactive Task, Wave, repository and bound conversations are work sessions,
as are conversations opened in a held terminal through Desktop or a framed
`lf --machine` view. Workers run headless Task Graph steps, the `ci-fix` run
landing starts, and Discord answers for a Wave. The checkout, `--task` and
`LF_AS` only decide Task membership: accounting and Task steers always travel
together. Resume retains a Session's bare or managed lifetime, reading its
binding and primary scope once before delegation; attribution alone leaves it
bare. Every launch [reads its own steers from the store](#every-launch-reads-its-own-steers).
Chats and batches report no boundaries. Work sessions and workers
report turn boundaries to the Machine server.

For a work session or worker, Claude and Codex hooks run `lf __context-block --moment <moment>` at session start,
compaction, each prompt and each turn end. That process first makes one
`session.boundary` request on `machine.sock`, for the server to record. The
OpenCode plugin and the Pi extension send the same request from inside the
provider ([below](#opencode-and-pi-report-from-inside-the-provider)):

```text
→ {"type":"hello","revision":7,"min_revision":7,"client":{"kind":"hook","platform":"macos","name":"lf __context-block","version":"0.15.1"}}
← {"type":"welcome",…}
→ {"type":"request","id":"stop","operation":{"kind":"session.boundary","moment":"stop","delivery":"/Users/me/.lf/context-deliveries/0b9f….json","adapter":{…},"callback":{"hook_event_name":"Stop","session_id":"5f1c…","prompt_id":"p-12",…},"gap":null,"printed":[],"inbox":{"claude":"/Users/me/.claude/sessions/4242.9c1d….key"}},"select":null,"request_id":null}
← {"type":"response","id":"stop","trace_id":"…","result":{"steers":null,"update":null,"waiting":false}}
```

`moment` is `start`, `compact`, `tool`, `stop` or, from the Pi extension,
`event`. `delivery` is the Session's
context delivery file, `adapter` the Session's provider context, `callback` the
JSON the provider sent the hook, unchanged. A conversation lf did not start,
such as a thread a Codex app-server opened, has no provider context: its hook
sends only `stop`, with a null `adapter`, and the server only refreshes
planning. `gap` is the hook's open recording
gap, `{first, last}`, or null. `inbox` is where the conversation
takes messages while idle, and names which kind it is. From Claude it is
`{"claude": key}`, the key file the conversation publishes for peers of its
cross-session inbox, `sessions/<pid>.<sha256 of socket>.key` in its
configuration directory, found from the `CLAUDE_CODE_MESSAGING_SOCKET` Claude
gives its children. From an OpenCode or Pi work session it is
`{"socket": path}`, the Unix socket its plugin or extension listens on. It is
null from Codex and from workers.

Before answering, the server records what the boundary says: the native turn
(started at a prompt, yielded at a turn end, with the open background tools and
scheduled wakeups Claude reports), the acknowledgement of the native input, and
a `store-gap` observation for a non-null `gap`. At
`stop` it answers first, then records a Codex Session's turns from its
app-server, refreshes the Session's planning, wakes the Session with an update
its turn missed, and starts any pending Session move, so none of it waits inside
the turn. The answer carries nothing: `printed` in the request and `steers`,
`update` and `waiting` in the answer are what conversations started before
hooks read their own steers still send and parse (LOO-838 deletes them).

The hook renders the context block from the delivery file itself, so a hook
that gets no answer still prints the block.

### Every launch reads its own steers

```bash
lf -b research 'how does src/sync.rs retry?' &
lf steer SESSION 'skip the vendored code'   # read after the child's next tool call
```

After a tool call, at a prompt, at session start and after a compaction, a
launch's hook opens the store, writes what changed on the Session's Tasks as
steers (`task_updates::write`) and claims the Session's unread steers
(`steer::claim`). One row in
`steer_delivered` per Session and steer says whether that Session read it:

1. The hook inserts the row with `claimed_at`, for up to eight steers, oldest
   first. A hook running beside it finds them claimed.
2. It prints them ahead of the block, with what changed on the Session's
   Tasks since its last update (Graph runs and steps, its Graph position,
   comments from anyone else). The Session's first read records the baseline
   and prints no update.
3. It stamps `delivered_at`.

A hook that dies between 2 and 3 leaves a claim that lapses after fifteen
seconds, and the steer is printed again: a steer can arrive twice, and cannot
be lost. `lf session steers <session>` shows each steer `delivered` or
`pending`. A store the hook cannot read is a
line in `logs/lf.log`; the tool goes on and the next boundary asks again.

The launch carries its capability as `LF_PROVIDER_CONTEXT` when the server
records its boundaries and as `LF_BARE_STEER` when no one does. For chats and
batches, tool and prompt hooks run only `--moment steer`, which reports nothing. Nothing wakes a chat or batch: a chat idle at its prompt reads a
steer when its person next types. A work session's start claims none when the
server can wake it; the wake runs them as a turn.

Every tool hook first compares files and starts `lf` only when a steer may be
waiting. Saving a steer touches one marker under `steers/` inside the write:
`session/<id>` for a Session's, `task/<id>` for a Task's, `latest` for a
Wave's or repository's. A launch names its Session's marker and its Task's
(`LF_STEER_SESSION`, `LF_STEER_TASK`); its hook exits while `LF_STEER_SEEN`,
touched at each check, is newer than all three. So a comment on one Task
starts `lf` only in that Task's conversations. A managed launch's prompt hook
always runs, which reaches a Session bound to a Task after launch. For chats
and batches, prompt hooks are gated like tool hooks.

### Waking an idle conversation

The server keeps no list of conversations (`wake.rs`); it reads them from the
store. A work session is idle when its last recorded native turn yielded
(`session_activity`) and its AgentProcess is live, and reachable through its
Codex app-server (`agent_sessions.agent_endpoint`) or what its boundaries
saved: a Claude inbox key in `agent_sessions.agent_inbox`, where
`sessions/<pid>.json` beside the key names the socket and must still name the
conversation, or the socket an OpenCode plugin or Pi extension listens on in
`agent_sessions.agent_socket`. One
server thread waits on the store's change signal (`store::changes`) and reads
the `task_updates` revision and the newest steer at start and whenever either
moves; every idle conversation with news gets its unread steers, Task updates
among them, in one message, and is recorded as in a turn until its next Stop.
One with no news is not connected to. A turn end checks
its own conversation the same way. The steers are claimed as a hook claims
them and stamped once the message is sent, so a failed send leaves a claim
that lapses for the next boundary:

- Claude: a message in its cross-session inbox, from `loopflow`, which Claude
  runs as the conversation's next turn. Managed Claude launches set
  `crossSessionInbound` to `accept`: lf has no permission mode to attest, and
  a session that bypasses permission prompts otherwise holds the message for
  review. Bare launches keep Claude's default.
- Codex: a `turn/start` on the Session's private app-server whose tool output
  is the message.
- OpenCode and Pi: one JSON line, `{"thread", "text"}`, on the socket the
  plugin or extension opened under `/tmp/lf-wake-…/` when its work session
  started. It asks its provider for a turn (`session.synthetic`; Pi's
  `sendMessage` with `triggerTurn`) and answers `started`. One that runs
  another conversation, or whose provider refuses, closes without answering.
  A turn already running takes the message as a steer. OpenCode's TUI shows
  the answer but not the message that caused it.

A conversation reopened after it was quit takes what was saved while it was
closed as a turn, before anyone types. A new provider process's first boundary
records it idle, whatever the process before it was doing
(`record_first_boundary`), and that boundary or a later one naming a new inbox
or socket wakes it. Claude reports its start; Codex runs its start hook only
inside a turn, so lf's launch reports the start itself once the app-server
holds the conversation, or the opening prompt before its turn. An OpenCode
plugin names its socket as it loads, a Pi extension at `session_start`. A
start the server recorded on a conversation it can wake claims no steers:
they go to the wake, and the next boundary claims what it could not deliver.

A server that restarts reads the same conversations again. An unreachable
Claude inbox or socket leaves its news unclaimed. After a failed send the steers arrive
again. Chats and batches are never woken. No
process or clock runs per Session: the watcher's only clock is the change
signal's 60 s fallback for a missed checkpoint or a Machine's sleep.

A managed launch's agent starts only once the server answers. The launch
opens a connection to the server, as a hook does; when nothing answers it runs `lf install server`
through the selected installation, as a managed view does, and asks again.
Still nothing, and the launch fails before the agent starts:

```text
Loopflow server is not running, and Task and Wave work needs it (…); run `lf install server`, then `lf server status`
```

A chat or batch, here or through `lf --machine`, runs without a server.
It prints the context block from checkout files at session start and compaction. Those
two callbacks make no request and log no gap, and the run reports no turn
boundary; whoever started it reads its output. Its steer check reads the
store directly. On Pi the extension still guards bash tools, which ends a
tool's process group with its provider. A chat in a Desktop shell has the
same hooks.

The hook resolves the server at its fixed path on every call; no address is
put in the agent's environment. Connecting, `hello` and the answer share a
two-second deadline. A refused connection, a missing socket, a timeout,
`incompatible`, `busy` or any `error` ends the report the same way: the hook
notes the gap once in `logs/lf.log`, reads its steers and exits 0.
A server that has not yet reached this build answers `invalid {kind}` for
`session.boundary`, which is the same gap.

### OpenCode and Pi report from inside the provider

A managed OpenCode or Pi turn with a dozen tool calls starts no `lf` process
between model steps unless a steer was saved for it. Context rendering still
starts `lf __context-block`: at OpenCode's first model request, at each Pi
turn's first model request, and after compaction in either provider. The
plugin runs inside the provider's own long-lived process, so at each boundary
it reports as a hook does without the process: it opens `machine.sock`, sends
the `hello` a hook sends, makes one request and closes the connection when the
answer arrives:

```text
→ {"type":"hello","revision":7,"min_revision":7,"client":{"kind":"plugin","platform":"macos","name":"lf pi extension","version":"0.15.1"}}
← {"type":"welcome",…}
→ {"type":"request","id":"boundary","operation":{"kind":"session.boundary","moment":"event","delivery":"…","adapter":{…},"callback":{"type":"tool_execution_end","turn":"7d0e…",…},"gap":null,"printed":[],"inbox":null},"select":null,"request_id":null}
← {"type":"response","id":"boundary","trace_id":"…","result":{"steers":null,"update":null,"waiting":false}}
```

The request has the same fields and deadline as a hook's. Boundaries go to
the server one at a time, in the order the provider reported them. The plugin
keeps its open gap in memory instead of the file beside the delivery file.
When context needs rendering, the plugin sends `start` or `compact`, then runs
`lf __context-block` without the launch's capability, which renders the block
from checkout files and asks no one; it reuses that block for every model
request until the next one.

For steers on every launch, the plugin makes the hook's file comparison in
process and starts `lf __context-block --moment steer` only when a marker is
newer than its last check.

- **Pi** sends each event it reports (session start, each message end, tool
  start and end, UI prompt start and end, and a `working` report at most
  every five seconds while the model streams) as `moment: "event"`, with the
  event as `callback`; the server records the turn from it. At each event
  while a turn runs it checks for steers and hands them to Pi as a steer.
  Outside a turn Pi would store a steer without starting a run, so it checks
  none. Turn end is `stop`. Under lf's RPC driver, which records Pi's events
  itself, the extension reports only `start`, `compact` and `stop`. While a
  gap is open, only session start, tool start and the turn end still wait up
  to two seconds; the other events are left out of the record and fall into
  the gap, and later events repeat the turn, open tools, pending input and
  assistant messages.
- **OpenCode** sends `tool` before each model request and puts any steer
  text ahead of the block. Turn end is `stop`.

Pi's bash guard is a `/bin/sh` script in the extension, not an `lf` process
(see `harness/context.rs`); it asks no server.

A connection that drops, or a request with no answer within two seconds,
is a gap like a hook's: the boundary goes on without lf, the plugin writes
one `recording gap` line to its log (`logs/pi-extension.log` for Pi,
`opencode.stderr.log` beside the capture for OpenCode) and nothing to the
conversation. The next boundary connects as every boundary does, sends the
open gap, and logs one line when it is answered. A
server restart, an upgrade and a stopped server all look like this; the
conversation needs no relaunch. A batch opens no connection: Pi
keeps its context block, steers and bash guard, OpenCode its context block and
steers, and so does a chat. A launch with no context delivery file has no
`delivery` to send and opens none either.

A store the server cannot write, or a callback whose conversation the Session
does not select, answers `unavailable`; the hook keeps its gap open and sends
it again with its next request.

A pending Session move starts two seconds after the `stop` answer, once the
hook has exited and the provider has ended its turn. A move that fails stays
pending for the next `stop`.

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

`task.shipped` means a PR merge was recorded, not that auto was turned on or
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
| `session.card` | Session, liveness, occupied terminal and last line |

Use `section` to place a Task in Backlog, Working, Done or
Abandoned; `state` describes its progress within that grouping. To populate a
board row's detail, request `task.detail`; for its Session, request `session.card`.
A card observes the Session: reading it does not resume a provider, attach a
terminal or add a view. Its `liveness` is `live`, `dead` or `unknown`;
`control_holder` is the recorded occupied terminal ID.
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
Session or live terminal.

`select` remains useful for scripts and agents that consume arbitrary JSON.
A selected `plan.list` response cannot be decoded as the board DTO, and
selecting fields out of any typed response can make it undecodable. Request
the screen read when a typed client needs a smaller payload. Desktop's
outline continues to use `plan.list` for its richer runtime data.

Handle failures through the [protocol error codes](#local-protocol-revision-7).
Check the named Task or Session when it cannot be found. If the host cannot
answer, retain any previous display as stale and retry the read after recovery;
an unavailable Session card does not establish that the Session ended.

## Reconnect a watch

After reconnecting and completing `hello`/`welcome`, resume the `session.list`
watch above with the checkpoint from its last applied reading:

```text
→ {"type":"subscribe","id":"b2","operation":{"kind":"session.list","directory":"/repo","headless":false,"after":null,"limit":null,"task":null},"select":["id","title","state"],"resume":"opaque-reading-token"}
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


Bulk fetches and oversize uploads use ticketed connections of their own. The phone
(`swift/LoopflowiOS/MachineConnection.swift`) keeps one control connection per
foreground session and the same ticket flow. It starts a fresh subscription with
`resume: null`, then replaces its cached Task rows with the current reading.

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
its Work; attaching to the Session's held terminal recovers its display. These are
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
one maximum-size ticketed message (32 MiB) before work starts, then holds the serialized
answer through delivery or ticket expiry. Admitted requests wait for producer
headroom before executing; cancellation during that wait runs nothing. Upload
tickets reserve their headroom before accepting a body. These provisional limits bound retained output, not
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
`run`, plus the primary Session's record (with `open_argv`) and the Task's `row`
in the Work projection's shape, read after commit, so a client selects the Task
and opens the conversation without another read. An admitted Task's row is
already started (Working when claimed). A repeat reads them again; the receipt
does not store them.

| Milestone | Means | Repeat |
| --- | --- | --- |
| `filed` | Task recorded, no `run` | complete |
| `admitted` | Task, claim, Graph and primary Session recorded in one transaction; the conversation sets the Task up | complete |
| `run_requested` | `-b` launch requested, unconfirmed | inspect |
| `started` | `-b` launch recorded | complete |

`admitted` involves no network and no Git beyond resolving the repository and
the Graph file. Tracker sync and the assignee check run after the answer under
the request's trace; their failures are Task events. Setup is the Task's, not
the receipt's: a conversation opening on a Task that took up its Graph and never
ran it makes the checkout and starts the Graph, and the receipt only answers
repeats. The held conversation carries `--request-id`, so its setup steps are
timed under the creation's request.

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

Terminal continuation addresses the held terminal, separately from Work
reads. A view resumes from its PTY byte position, served from the holder's
2 MiB ring; a server restart does not invalidate the position, and one older
than the ring falls back to a fresh screen. Uncertain input is never
replayed. A recovering view attaches beside the others; every view types, and
the one used most recently sizes the terminal. A stalled bridge cannot block another viewer or Work control traffic.

If the Session has moved to another terminal, attaching to the old one
returns `not_found` with `terminal_id` and `current_terminal_id`. Attaching by
Session reaches the current one; neither attachment starts or resumes a
provider. A lost
connection alone does not establish process death.

Desktop reconnects to the same held terminal with bounded backoff. See
[connection loss](../../../../docs/server.md#keep-working-through-a-connection-loss)
for connection status and recovery controls outside terminal bytes.

The Loopflow Session, provider conversation, serving AgentProcess and the
views attached to its terminal are distinct. `SessionRecord.transition` reports the latest
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

Large answers retain their connection and process memory reservations until the
ticket is downloaded or expires. Expiry releases abandoned downloads without
waiting for another request. Events remain within the control-message limit;
clients check connection-wide sequence before deferring delivery behind a bulk
reading.
