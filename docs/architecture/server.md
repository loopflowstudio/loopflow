# Clients, servers and execution destinations

**Accepted design, November 4, 2026.** Jack Heart selected separate Machine,
server and installation responsibilities. The local backend exists in LOO-593;
the command and source-name migration below is pending. Proposed commands on this
page are not yet executable examples. LOO-718 supplies the mobile client and
remote-access extension.

## Start from the action

| Action | Concept | Surface |
| --- | --- | --- |
| Run an agent on a different computer | Execution destination | Existing `lf machine` connections, pools and rentals; `lf --machine` routes the command |
| Read Work or continue a Session from Desktop or a phone | Client connection to Loopflow on a host | Proposed `lf server`; shared reads, commands and terminal bridges |
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
Desktop ── private Unix socket ──┐
                              ├─ server ─ shared Rust reads/subscriptions
phone ─ authenticated HTTPS ──┘         ├ complete CLI commands
       / WSS (LOO-718)                  └ existing Session owner ─ provider / PTY

lf --machine host … ─ SSH ─ destination's ordinary lf command
lf install ─ selected artifacts + OS registration of server
lf desktop ─ app-owned automation socket ─ windows / panes
```

The server owns client admission, compatibility, framing, bounded delivery and
connection diagnostics. Shared commands own resolution, defaults, mutation,
attribution and recoverable outcomes. Session owners own providers, PTYs,
terminal output and input/geometry claims. Graph processes own Graph execution.
CLI operations can run without the server. Desktop owns rendering and UI actions.

Keep one implementation of each complete operation. Remove migrated Work FFI
paths instead of keeping a silent fallback; preserve renderer FFI and app-owned
automation. A shared Rust function alone does not prevent duplicated orchestration.
The server adds no scheduler, Work database, viewer registry or provider supervisor.

Local peer credentials apply only to the private Unix socket. Remote access needs
its own authenticated admission even over Tailscale. Never forward a trusted local
socket and treat the proxy's UID as the phone's identity. Disabling remote access
or revoking a client must preserve local access and existing execution owners;
client credentials grant no permission to disregard owner control claims.

## Installation owns registration

One installation authority selects the executable, reconciles its OS registration
and replaces it on update. Desktop can request readiness through that authority;
it cannot install a competing helper. Server status observes readiness without
installing or repairing anything. Recovery reconnects; it does not re-register a
disabled background item or change the selected build.

The current macOS package uses a signed app's bundled helper with SMAppService;
Linux uses a user systemd service. Development data directories have separate
supervisor identities. The registration code's current location under
`machine_service/` does not make it Machine routing's responsibility.

Server replacement must leave detached Session and Graph owners alive under the
actual OS supervisor. `setsid()` alone proves no such isolation. The signed macOS
same-build replacement proof covers a held draft and running Graph; Linux
replacement and rendered Desktop continuity remain separate acceptance work.
Sleep makes the host unavailable; logout and reboot do not preserve live
processes. Before-login hosting and a GUI-less distribution remain separate
product choices. Removal must preserve Work and credentials.

## Command cutover

The proposed public surface is `lf server run` for the supervisor's foreground
entry and `lf server status` for diagnostics. Remote enablement, pairing and
client revocation belong to that group too; their final subcommand spellings
must be settled with LOO-718. Installation remains `lf install`.

Current LOO-593 uses `lf machine service run`, `status` and `ensure`.
Replace those paths in the CLI tree, routing, supervisor definitions, installed
launcher, Swift adapter, fixtures and documentation together. Registration and
the installation part of `ensure` move behind installation; readiness belongs to
the server client. Do not simply rename `ensure` into another public installer.
Remove the old internal command routes after their callers migrate.

Rename implementation concepts such as `machine_service`, `MachineService` and
`MachineProtocol` to server concepts in the same coordinated cutover. Keep
`MachineId`, `machine_id`, `LF_HOME`, destination routing and Work ownership:
those still describe the host authority correctly. Coordinate wire/fixture
changes with both clients; namespace cleanup alone needs no protocol revision.

Installed OS labels and persisted configuration require explicit migration:
replace the old registration without leaving two listeners, preserve remote
access settings and paired credentials, and do not retire busy owners' executables.
LOO-718 must move its remote-access and pairing commands with this boundary.
The [current local protocol](../../rust/loopflow/src/machine_service/README.md)
retains executable commands and exact source names until that work lands.
