# Clients, servers and execution destinations

Machine routing, client access and installation have separate owners. The local
client connection is implemented; LOO-718 covers the mobile client and remote
admission.

LOO-819 plans the server holding managed Sessions' processes:
[Server-first Sessions](server-first.md).

## Start from the action

| Action | Concept | Surface |
| --- | --- | --- |
| Run an agent on a different computer | Execution destination | Existing `lf machine` connections, pools and rentals; `lf --machine` routes the command |
| Read Work or continue a Session from Desktop or a phone | Client connection to Loopflow on a host | `lf server`; shared reads, commands and terminal bridges |
| Install or update Loopflow | Installation | Existing `lf install`; selected artifacts, server registration and replacement |
| Focus a window or arrange its panes | Desktop presentation | Existing `lf desktop`; the app-owned automation socket |

A Machine remains one OS user's Loopflow authority in one data directory. Its
identity, credentials, Work and execution history survive server replacement.
The server is a replaceable access process on that Machine. A paired phone is a
client credential, not another execution destination or another Machine record.

## One concrete path

Install Loopflow, open Desktop, and keep editing a Session. Installation prepares
the local server; Desktop connects without a separate setup command. Remote
client access and phone pairing are planned in LOO-718.
Connecting a client does not rent a worker, move a Task, or select another provider.

If the server is replaced during an unfinished draft and a running Graph,
Desktop retains the screen and local reading actions. Native status shows the
connection recovering; input pauses until the same owner acknowledges. A newer
viewer's control claim survives automatic recovery. **Move here** explicitly
requests control from that owner. A lost acknowledgement is never a reason to
repeat takeover or replay input. **Retry** repairs the connection only.

The server may become unavailable while execution continues. Only the owner can
establish that the Session ended. Existing owners retain their lifetime policy:
drafts, active turns, pending requests and running Graphs already protect their
owners. An API connection introduces no new retention authority.

## Boundaries through the implementation

```text
Desktop ── private Unix socket ── server
phone ─ authenticated HTTPS/WSS ─ server (LOO-718)

server read/subscription ─ shared Rust readers
server write ─ request worker ─ Write::run ─┐
CLI mutation ─ mutation dispatcher ─────────┴─ engine operations / shared Rust write functions
server task.run, task.ship ─ engine::launch ─ detached `lf task run`, `lf ship` ─ Graph owners
server attach ─ exact existing Session owner ─ provider / PTY

Desktop terminal preparation/launch ─ ordinary CLI ─ existing Session owners
lf --machine host … ─ SSH ─ destination's ordinary lf command
lf install ─ selected artifacts + OS registration of server
lf desktop ─ app-owned automation socket ─ windows / panes
```

The server owns client admission, revision negotiation, framing, bounded
delivery and connection diagnostics. Shared commands own resolution, defaults, mutation,
attribution and recoverable outcomes. Session owners own providers, PTYs,
terminal output and input/geometry claims. Graph processes own Graph execution.
CLI operations can run without the server. Desktop owns rendering and UI actions.

Subscriptions carry complete readings of current state and typed events for
occurrences such as a Task shipping, a steer arriving or a Graph step finishing.
Clients can use an event payload to animate a change or append a thread entry
without re-reading. Events have connection-local sequence numbers and no replay
cursor; a gap or reconnect requires a fresh reading before incremental updates
continue. The [event contract](../../rust/loopflow/src/server/README.md#events-alongside-readings)
defines sequencing and recovery alongside reading checkpoint resumption.

Every shared native Work read, write and subscription uses this server. Desktop
links no Loopflow Rust library or C bridge. Served writes and CLI handlers call
the same shared Rust functions, which return typed values: `Write::run`
(`lf/commands/write.rs`) keeps each write's declared result type through
dispatch, and that type supplies its `x-result` schema. The server serializes
the value into `response.result`, and the CLI
(`lf/commands/mutation.rs`) prints it with `--json` or renders it as text. The
server runs a write in a request-scoped worker with the request's directory,
without starting a CLI child, parsing standard output or changing
process-global cwd, environment or standard streams.

Served writes create no Process rows and inherit no launcher Session or Task
attribution. `journal::request` carries tracing and diagnostics into nested
blocking work; the response trace correlates diagnostics and effects in
`logs/lf.log`. It is neither an execution owner nor a durable retry receipt.
Canceling releases the client's wait, but an admitted mutation may still complete.
A lost reply means outcome unknown. For `task.create` and `session.new`, look up
the durable creation receipt. Its derived `disposition` is `complete`, `continue`
or `inspect`; only confirmed absence or `continue` permits resubmitting the
identical request. The handler rechecks continuation under its request lock. For other writes,
refresh authoritative state before another deliberate edit. The
[client contract](../../rust/loopflow/src/server/README.md#client-contract)
defines persistence, receipt lookup, admission budgets and recovery by error scope.

`task.create` shares its creation function and receipt with the CLI, covering
both new Tasks and Session adoption; `run` requests the Task's Graph launch.
`task.run` and `task.ship` start the same command a terminal would, detached
under its own execution owner, and answer the typed value that process reports
(`engine/launch.rs`); the request never carries the run. Desktop's Work operations
use the shared protocol; execution stays with existing Session/Graph owners.
Ghostty rendering and app-owned automation retain
their boundaries.
The server adds no scheduler, Work database, viewer registry or provider supervisor.

Local peer credentials apply only to the private Unix socket. Remote access needs
its own authenticated admission even over Tailscale. Never forward a trusted local
socket and treat the proxy's UID as the phone's identity. Disabling remote access
or revoking a client must preserve local access and existing execution owners;
client credentials grant no permission to disregard owner control claims.

## One namespace in three places

An operation's API kind, its `lf` spelling and any setting for it share one
name (`task.create` is `lf task create`); the nouns are Loopflow's primitives.
Old spellings live in `lf/aliases.rs` for one release.

The [operation reference](../../rust/loopflow/src/server/README.md#local-protocol-revision-5)
lists the served kinds and their CLI equivalents. `lf server describe --json`
generates their schemas and result types from this build. App automation remains
CLI-only; a served operation calls the same Rust owner as its CLI equivalent.

For a board, Task detail or Session card, use the
[screen reads](../../rust/loopflow/src/server/README.md#read-a-screens-data).
Each returns its own complete DTO in both the CLI and API. Swift clients send
`select: null`; field selection is for clients that consume arbitrary JSON.
Desktop's outline keeps `plan.list` for its runtime data.

## Installation owns registration

One installation authority selects the executable, reconciles its OS registration
and replaces it on update. Desktop can request readiness through that authority;
it cannot install a competing helper. Server status observes readiness without
installing or repairing anything. Recovery reconnects; it does not re-register a
disabled background item or change the selected build.

The current macOS package uses a signed app's bundled helper with SMAppService;
Linux uses a user systemd service. Development data directories have separate
supervisor identities. Registration lives under `installation/`, outside Machine routing.

Server replacement must leave detached Session and Graph owners alive under the
actual OS supervisor. `setsid()` alone proves no such isolation. Validate owners
launched through the replaced executable, recovery acknowledgement, submission
of the retained draft and subsequent Graph progress. Final macOS replacement,
Linux supervision and rendered Desktop continuity remain acceptance work.
Sleep makes the host unavailable; logout and reboot do not preserve live
processes. Before-login hosting and a GUI-less distribution remain separate
product choices. Removal must preserve Work and credentials.

## Commands and installed-state migration

`lf server run` is the supervisor's foreground entry. `lf server describe --json`
prints the protocol's JSON Schema, generated from the wire types, with no server
running. `lf server status` reports
readiness without registering a helper. The hidden `install server` operation
lets local clients request readiness from the selected installation; it does not
select or install a different build. Machine has no service subcommands.

Rust `server/`, Swift `ServerClient` / `ServerProtocol` and the shared fixtures
under `tests/fixtures/dto/server/` speak revision 5: one connection per client,
a negotiated revision, requests multiplexed by id, newline-delimited JSON on the
Unix socket. A revision-1 client is answered in its own framing with
`incompatible` and nothing else. Machine identity fields, `LF_HOME`, SSH routing
and Work ownership are unchanged. Native terminal relays use `LF_SERVER` and
`LF_SERVER_STATUS`.

Installation retires the old supervisor job before registering the server job.
The old bundled plist is only an SMAppService migration descriptor: inspect
approval, then unregister; never register it from new code. Disabled choices
remain respected. Custom-home jobs and systemd units migrate their hashed labels;
the `machine.sock` address and singleton lock stay stable. Relays from before
the namespace change can recover to the same listener; old and new executables
cannot listen concurrently. No socket alias or second path exists.
No Work, credential or remote-setting files move in this local cutover.

LOO-718 must move remote admission/pairing commands under `lf server` while
retaining its credential/settings storage. The [local protocol](../../rust/loopflow/src/server/README.md)
owns operation shapes and bounds. The working design records the precise handoff
and acceptance still owed by the combined release.
