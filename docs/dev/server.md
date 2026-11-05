# Clients, servers and execution destinations

Jack Heart selected separate Machine, server and installation responsibilities
on November 4, 2026. LOO-593 implements the local boundary; LOO-718 supplies the
mobile client and remote admission.

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
the local server; Desktop connects without a separate setup command. To continue
from a phone, explicitly enable remote client access and pair it with the host.
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

server read/subscribe ─ shared Rust readers
server write ─ request worker ─ Write::run ─┐
CLI mutation ─ mutation dispatcher ─────────┴─ engine operations / shared Rust write functions
server task.run, task.ship ─ engine::launch ─ detached `lf task run`, `lf ship` ─ Graph owners
server attach ─ exact existing Session owner ─ provider / PTY

Desktop execution launch ─ ordinary CLI ─ existing Session / Graph owners
lf --machine host … ─ SSH ─ destination's ordinary lf command
lf install ─ selected artifacts + OS registration of server
lf desktop ─ app-owned automation socket ─ windows / panes
```

The server owns client admission, revision negotiation, framing, bounded
delivery and connection diagnostics. Shared commands own resolution, defaults, mutation,
attribution and recoverable outcomes. Session owners own providers, PTYs,
terminal output and input/geometry claims. Graph processes own Graph execution.
CLI operations can run without the server. Desktop owns rendering and UI actions.

Every shared native Work read, write and subscription uses this server. Desktop
links no Loopflow Rust library or C bridge. Served writes and CLI handlers call
the same shared Rust functions, which return typed values: `Write::run`
(`lf/commands/write.rs`) returns the value as the response, and the CLI
(`lf/commands/mutation.rs`) prints it with `--json` or renders it as text. The
server runs a write in a request-scoped worker with the request's directory,
without starting a CLI child, parsing standard output or changing
process-global cwd, environment or standard streams.

Served writes create no Process rows and inherit no launcher Session or Task
attribution. `journal::request` carries tracing and diagnostics into nested
blocking work; the response trace correlates diagnostics and effects in
`logs/lf.log`. It is neither an execution owner nor a durable retry receipt.
Canceling releases the client's wait, but an admitted mutation may still complete.
A lost reply means outcome unknown: refresh authoritative state before another
deliberate edit, and never automatically replay the request.

Session-to-Task adoption (`task.create`) shares its creation function with the CLI and launches no work.
`task.run` and `task.ship` start the same command a terminal would, detached
under its own execution owner, and answer the typed value that process reports
(`engine/launch.rs`); the request never carries the run. Create-and-run and
Session launches retain their ordinary CLI path and existing Session/Graph owners. Ghostty rendering and app-owned automation retain
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

| API kind | CLI | Engine |
| --- | --- | --- |
| `task.get` | `lf desktop task <issue>` (`lf task show` pending) | read |
| `task.list` (`closed: true`) | `lf desktop done --wave <wave>` (`lf task list --closed` pending) | read |
| `task.changes` | `lf task diff <issue> --files` (`lf task changes` pending) | read |
| `task.diff`, `task.file` | `lf task diff`, `lf task file` | read |
| `task.context` | `lf session usage --days 0 --task <task> --context` (`lf task context` pending) | read |
| `task.runs` | `lf graph list --processes --for-task <task>` (`lf task runs` pending) | read |
| `task.wait` | `lf task wait <issue> --until <state>` | `engine::task` |
| `task.create` | `lf task create --session <id>` | write |
| `task.run` | `lf task run <issue>` | `engine::task` |
| `task.ship` | `lf ship` | `engine::task` |
| `task.hold` | `lf ship --stop` (`lf task hold` pending) | `engine::task` |
| `task.steer` | `lf task comment <issue> <message> --steer` (`lf task steer` pending) | `engine::task`; proxy to `session.steer` pending |
| `task.comment` | `lf task comment <issue> <message>` (`task.report`, `lf task report` pending) | `engine::task` |
| `task.interrupt` | `lf task interrupt <issue>` | `engine::task` |
| `task.update` | `lf task update <issue>` | write |
| `task.move`, `.claim`, `.unassign`, `.save`, `.complete`, `.reopen`, `.abandon` | `lf task <verb>` | write |
| `session.list` | `lf desktop sessions` (fold into `lf session list` pending) | read |
| `session.owner` | none | server |
| `session.usage` | `lf session usage` (`lf usage`) | read |
| `session.history` | `lf history` (`lf session history` pending) | read |
| `session.retention` | `lf session retention` | read |
| `session.rename`, `.bind`, `.resume`, `.new` | `lf session <verb>`, `lf resume` | write |
| `session.steer` | `lf session steer` | CLI only, engine pending |
| `wave.list`, `wave.graph` | `lf wave list`, `lf wave graph show` | read |
| `wave.planning` | `lf desktop wave <wave>` (`lf wave planning` pending) | read |
| `wave.create` | `lf wave create` | write |
| `wave.sync` | `lf wave status <wave> --sync` (`lf wave sync` pending) | write |
| `wave.planning.bind`, `.connect`, `.reteam` | `lf wave bind-project`, `lf wave connect`, `lf wave reteam` | CLI only, engine pending |
| `planning.list` | `lf desktop plan` (`lf planning list` pending) | read |
| `repository.planning` | `lf repository planning` | read |
| `repository.people`, `.tokens`, `.path` | `lf repository people`, `lf repository tokens`, `lf repository path` | read |
| `repository.release`, `.ci` | `lf repository release`, `lf repository ci` | CLI only, engine pending |
| `graph.list`, `graph.customize` | `lf graph list`, `lf graph customize` | read, write |
| `graph.run` | `lf graph show <id> --processes` | read |
| `machine.self`, `.list`, `.doctor` | `lf machine self`, `lf machine list`, `lf machine doctor` (`lf doctor`) | read |
| `process.list` | `lf monitor ps` (`lf process list` pending) | read |
| `process.attach` | none; the CLI reaches an owner's socket directly | server |
| `work.resolve` | `lf open <url> --link` | read |
| `config.catalog`, `.settings`, `.get`, `.explain` | `lf config catalog`, `lf config settings`, `lf config get`, `lf config explain` | `engine::config` |
| `config.set`, `.unset` | `lf config set`, `lf config unset` (`lf set`, `lf unset`) | `engine::config` |
| `config.theme` | none; Appearance's theme file | `engine::config` |
| `pr.*` | `lf pr publish`, `open`, `submit`, `arm`, `land`, `abandon`, `reconcile`, `checks`, `reviews`, `request-review` | CLI only, engine pending |
| `checkout.commit`, `.sync`, `.create` | `lf commit`, `lf sync`, `lf wt create` | CLI only, engine pending |
| `account.*` | `lf account …` | CLI only, engine pending |
| `skill.run`, `skill.list` | `lf <skill>`, `lf list` | CLI only, engine pending |
| `desktop.portfolio`, `.portfolio.set` | `lf portfolio` | read, write |
| `desktop.layout`, `.layout.save` | `lf desktop layout <work> [--save]` | read, write |
| `desktop.view.list`, `.view.get` | `lf desktop plans`, `lf desktop view --plan` | read |
| `desktop.dashboard.*` | `lf desktop query`, `pin`, `unpin`, `rename`, `hide`, `show`, `move -p <board>/<member>`, `layout query:<id> --save` | write |
| `desktop.<verb>` | `lf desktop enter`, `send`, `send-key`, `capture`, `zoom`, `focus`, `close`, `reopen`, `move`, `resize`, `browse`, … | CLI only, executed by the app |

"Pending" names the canonical spelling the engine design assigns that this
branch has not moved yet; `scratch/questions.md` records why.

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

`lf server run` is the supervisor's foreground entry. `lf server status` reports
readiness without registering a helper. The hidden `install server` operation
lets local clients request readiness from the selected installation; it does not
select or install a different build. Machine has no service subcommands.

Rust `server/`, Swift `ServerClient` / `ServerProtocol` and the shared fixtures
under `tests/fixtures/dto/server/` speak revision 2: one connection per client,
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
