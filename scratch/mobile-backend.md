# Separate execution destinations, client access and installation

LOO-593 · Implementation selected November 4, 2026 · Pursue → interactive demo

## Accepted direction

Jack Heart requested returning to the drawing board after identifying that
`lf machine service` mixes remote execution management with client API access.
Jack selected this split, then requested updating the design and relaunching pursue:

- **Machine** names where work runs: identity, SSH connections, destination
  defaults, pools and rentals. Keep `lf machine` and `lf --machine` here.
- **Server** gives Desktop and mobile access to Loopflow on a host. Move client
  admission, availability and the serving entry point to `lf server`.
- **Installation** selects and updates artifacts and registers the server.
  Keep that lifecycle under `lf install`, including automatic local setup.
- **Desktop** owns its windows, panes, rendering and app-owned automation API.
  Keep `lf desktop` and `DesktopServer` separate from the shared server.

The canonical boundary and usage are in
[Clients, servers and execution destinations](../docs/architecture/server.md).
This replaces the earlier Machine-service framing. Jack Heart now authorizes
implementation and publication of this cutover in the existing Task, followed by
interactive review. The earlier docs-only boundary is superseded. No merge or
Auto-Ship approval is given. The existing feature graph runs its pursue
composition through implementation, compression, sync, realignment, concept
review and publication, then stops at demo. A bare pursuit reaching end would
violate this accepted review boundary.

## Implementation status and remaining work

The local cutover is implemented in this checkout: `lf server run/status`, hidden
installation readiness, renamed Rust/Swift protocol clients, environment flags,
DTO fixtures and packaging. Registration is under `installation/`; no serving or
setup subcommand remains on Machine. Desktop caches its endpoint after initial
readiness; retries only reconnect. The protocol wire fields remain revision 1.

Upgrade retires old OS labels before registering the server. The native bundle
retains only an old plist migration descriptor; its obsolete argv cannot start a
new server. Custom-home launchd migration checks both disabled labels, and systemd
migration respects existing disabled/masked units. The installed revision-1 socket address/lock stay stable: changing the basename
would strand live relays running the prior executable during replacement.
One endpoint and its existing singleton lock exclude competing listeners.
No Work or credential state moves. A focused migration check found and fixed the
old launchctl parser's assumption that overrides print `true`; current macOS
prints `disabled`.

Remaining through the existing pursue composition:

- Compress/sync/realign against overlapping installation work, preserving this
  boundary and the existing recovery semantics. No wider FFI or remote cutover.
- Gate the affected command, protocol, relay, Swift and packaging boundaries;
  repeat the composed signed installed replacement with draft/Graph live, and
  run formatting/Clippy and the broader Xcode link check. The focused checks
  below do not replace gate or the combined LOO-718 release proof.
- Publish to existing PR #1813 and stop at interactive demo. Native rendered
  recovery, reading/copy/Find/scroll and Move here remain demo evidence. Linux
  supervisor replacement remains Linux-host acceptance. No merge or Auto-Ship.

Current open overlap inventory (November 4):

- [Native phone client · LOO-718 / PR #1812](https://github.com/loopflowstudio/loopflow/pull/1812)
  extends the same server modules and wire with remote admission, pairing and
  create/start. Preserve its shared contract; supply the migration map for its
  integration. Do not overwrite its checkout or duplicate remote implementation.
- [Coherent installed build · LOO-723 / PR #1815](https://github.com/loopflowstudio/loopflow/pull/1815)
  changes install and published-artifact selection. Keep its selection/update
  authority intact; reconcile new landed code during sync instead of forking it.
- [Task shell lifetimes · LOO-717 / PR #1811](https://github.com/loopflowstudio/loopflow/pull/1811)
  touches Machine commands. Preserve its shell lifetime and remote routing work.
- [Preferences and agent selection · LOO-714 / PR #1805](https://github.com/loopflowstudio/loopflow/pull/1805)
  touches published installation. Preserve preference/default behavior.
- [Mac installation · LOO-312 / PR #1656](https://github.com/loopflowstudio/loopflow/pull/1656)
  changes installation and onboarding. This pursuit changes server registration,
  not its product choices or first-run account setup.

Re-read overlap state before integrating; this inventory is evidence, not
permission to change another Task's work. Refresh Wave terminology only through
the graph's authored realign procedure.

## Delete — do not maintain

- Removed `MachineCommand::Service` and `MachineServiceCommand`; `ServerCommand`
  owns serving/status and hidden `install server` requests installation readiness.
- Moved `machine_service/installation.rs` and `registration.m` into
  `installation/`; the remaining module is `server/`, with old exports removed.
- Renamed Swift service/protocol/connection types, transport environment flags
  and the DTO fixture directory together; Machine identity fields are unchanged.
- Replaced old supervisor jobs with server names; the installed socket
  address/lock remains so live older relays still reconnect. The bundled old
  plist is retained only as an SMAppService migration descriptor, never registered
  by new code; it is required to inspect approval and unregister existing jobs.
  Remove that descriptor when support for upgrading pre-cutover installations ends.
- Removed old public setup examples; owner continuity and read-only terminal
  coverage remain on the surviving routes.

## Experience and ownership

Install Loopflow, open Desktop and edit a Session. Local client access is prepared
by installation; no server setup ritual or Tailscale is needed. Enable remote
access separately to pair a phone with that host. A phone is a client of a Machine,
not another execution destination. Selecting another worker remains an explicit
Machine operation.

```text
Desktop ─ private Unix socket ─┐
                             ├─ server ─ shared Rust Read / Subscription
phone ─ authenticated API ────┘         ├ complete command through installed lf
        (LOO-718)                      └ existing Session owner ─ agent / PTY

installation ─ selected executable + server registration / replacement
Machine routing ─ SSH destination's ordinary lf invocation
Desktop automation ─ app-owned windows / panes / menus
```

The server handles admission, framing and bounded delivery. Complete shared
commands retain defaults, mutation, attribution and recovery. Session owners
retain provider processes, input/geometry claims and lifetime policy. Graphs
retain their own execution. Drafts, active turns, pending requests and running
Graphs already protect their owners; the server adds no retention authority.
CLI use does not require the server. No scheduler, shadow Work store or durable
viewer registry belongs here.

## Local contract and mobile integration

LOO-593 implements a local server under `server/`, with roadmap
read/watch, Session rename through the CLI and exact existing-owner attachment.
Those migrated operations have no direct-FFI fallback. Other Work operations
still use their existing adapters. App-owned UI automation remains separate.

LOO-718/PR #1812 incorporates `07a6a184d4`; inspection at `983b271695` on November 4
found remote HTTPS/WSS over host Tailscale, pairing/revocation, native iOS and
recoverable create/start work. That is evidence from the parallel branch, not
verification of the combined release. LOO-593 is the local foundation; LOO-718
owns the remote/mobile experience. Both need the accepted naming boundary.

| Previous path | Current path / authority |
| --- | --- |
| `lf machine service run/status` | `lf server run/status` (`ops server` in the canonical tree) |
| `lf machine service ensure` | Hidden `lf install server`; installation readiness only |
| `machine_service/{installation.rs,registration.m}` | `installation/{server.rs,registration.m}` |
| Remaining `machine_service/` module | `server/` with unchanged revision-1 operation shapes |
| `MachineService`, `MachineProtocol`, `MachineConnection` | `ServerClient`, `ServerProtocol`, `ServerConnection` |
| `LF_MACHINE_SERVICE`, `LF_MACHINE_STATUS` | `LF_SERVER`, `LF_SERVER_STATUS` |
| `tests/fixtures/dto/machine/` | `tests/fixtures/dto/server/` |
| Installed `machine.sock` / old OS job labels | Socket/lock stay stable for live relays; OS jobs migrate to server labels, with only the old bundle descriptor retained for migration |

### LOO-718 integration handoff

Read-only inspection of its current open PR head `983b271695` confirmed
`MachineServiceCommand::{Mobile,Pair,Devices,Revoke}` and
`Run { remote_port }`. Move those variants to `ServerCommand` and dispatch via
`lf/commands/server.rs`. Keep the existing verbs/flags as `lf server mobile`,
`pair`, `devices`, `revoke` and `run --remote-port`; remove its `Ensure` variant.
Move `admission.rs`, `mobile.rs`, `remote.rs` with the server module; route its
registration changes through `installation/server.rs`. The existing `mobile`
key in `LF_HOME/config.yaml`, SQLite pairing/device rows, ticket hashes and Serve
route ownership remain unchanged. Rename its additional DTO fixtures and Swift
protocol references with this foundation; preserve every wire field including
`machine`/`machine_id`. No namespace-only protocol revision is needed.

This handoff does not modify LOO-718's checkout or import its schema/remote paths.
LOO-723/PR #1815 was still open at this pass; sync owns integration if it lands.

Do not create compatibility aliases for internal commands or duplicate protocol
implementations. Persisted state and installed supervisor registrations need an
explicit migration. Do not rename `MachineId`, `machine_id`, execution-location
records, `LF_HOME` or ordinary SSH routing just because the server reports them.
The namespace change alone does not require a new wire revision.

## Preserve the working contract

[The local protocol README](../rust/loopflow/src/server/README.md) owns
revision-1 operations and bounds. Rust wire types, the Swift mirror and fixtures
under `tests/fixtures/dto/server/` supply both clients. `view_plan`
returns authoritative `contents[].primary_session`; roadmap membership cannot
select a primary. Owner resolution never starts a provider. Attach names the
exact existing owner and bridges its unchanged binary protocol.

Reads/subscriptions reuse shared Rust operations and fresh complete readings.
Closing cancels; reconnect does not replay store revisions. Commands run complete
CLI operations with isolated environments and fresh traces. Request IDs correlate;
they do not deduplicate. A lost command reply means outcome unknown and cannot
be automatically resubmitted. Recoverable create/start belongs in shared command
admission, not a gateway response cache; preserve the original Task on launch
failure and never duplicate creation after a lost reply.

The private same-user Unix socket and remote admission have different trust
boundaries. A remote proxy cannot borrow its local UID to authenticate a phone.
Remote access is opt-in; revoking a phone or disabling it must leave local clients
and execution owners intact. Preserve unrelated Tailscale Serve routes. Any future
hub is optional ingress/retransmission, not the execution owner or a prerequisite
for private use.

## Preserve automatic recovery and explicit control

Jack Heart's earlier accepted review direction remains intact: automatic
exact-owner recovery, native connection status outside terminal bytes, no replay
of uncertain input and no replacement provider. The implemented native surface
has a private temporary mailbox; it carries status and one-shot actions only.
Closure removes it and cancels recovery. Eight capped-backoff attempts lead to
an actionable unavailable state and Retry. Unavailable is not ended: only the
owner's execution evidence establishes exit.

Input pauses until the exact owner acknowledges. Passive recovery requests free
claims without replacing a newer viewer. Terminal screens refresh; byte streams
resume their received position or refresh when the spool expired. Outage input
and uncertain writes are discarded. Native read-only mode preserves selection,
copy, Find and scrolling. Plain CLI server relays retain their existing Ctrl+R
interaction; Desktop uses native status.

**Move here** is available in the connected read-only pane. Its mailbox action
names the current generation and is consumed before the existing replacing
Attach is sent. Consumption advances the generation; input waits for the owner's
acknowledgement. Refresh the parser/screen together while retaining cumulative
stream offset. A lost acknowledgement recovers non-replacing, even if a newer
viewer has meanwhile acquired control. Never replay takeover. Closure cancels
pending actions; stale readings cannot re-enable consumed actions.

This counterexample remains decisive: a takeover succeeds, its reply is lost,
and another viewer claims control. Automatic recovery must leave that newer
viewer in control. Retry changes connectivity; Move here changes control.

## Installation and lifetime

One installation authority reconciles the selected executable, public CLI and
server registration. Desktop requests readiness from it; status and reconnect
cannot silently install, select another build or override disabled background
items. Current macOS packaging uses a signed app's bundled helper and SMAppService;
explicit development data directories have separate supervisor identities.
Linux uses a user systemd service with normal control-group cleanup.

Executable replacement may unregister and terminate the server. Existing owner
and Graph processes must survive actual supervisor cleanup; `setsid()` alone is
insufficient proof. The server must not become their launch/lifetime owner as
more commands move onto it. Execution-start operations require an isolation proof.
Sleep makes a host unavailable; logout/reboot preserve records, not live processes.
Before-login hosting and a separately distributed GUI-less package remain open
product questions. Removal must preserve Work/credentials and explicitly address
active execution; deleting Desktop implies no standalone hosting promise.

## Evidence and remaining proof

The socket contract test now owns read/write parity, watch checkpoints and trace
isolation once. Restart, installed replacement and Move here each exercise the
same smaller owner/draft/Graph continuity fixture. Packaging copies the authored
LaunchAgents directory, including the migration descriptor, without repeating
its file inventory in the release and development builders. Server fixtures now
initialize their disposable Machine store explicitly: the observational entry
correctly refused bootstrap when the previous fixture started it before any Work.

November 4 focused checks: `cargo test -p loopflow --lib server` (8), the
`transport_tests` status and opt-in registration migration tests (1 each),
`scripts/test_desktop.sh --filter 'Server(Connection|Protocol)Tests'` (7),
`uv run python scripts/test_server_registration.py` and
`uv run pytest python/tests/test_release_loopflow.py` (13) passed; `cargo fmt`
and `git diff --check` passed. The migration test was repeated after retaining
the installed socket address. Compression verification: `cargo test -p loopflow --test transport_tests -- server_reads_commands_and_subscriptions_share_cli_semantics server_invalid_checkpoints_use_shared_watch_errors server_preserves_owner_and_graph_across_restart server_move_here_preserves_draft_provider_and_graph server_status_does_not_install_or_create_work` — 5 passed; `uv run pytest python/tests/test_release_loopflow.py python/tests/test_loopflow_dev.py -q` — 22 passed; formatting, Clippy (`--all-targets -- -D warnings`), Ruff and whitespace checks passed. Gate owns remaining affected suites, broader Xcode link and the renamed
`installed_server_preserves_owner_and_graph_across_replacement` composition;
demo owns rendered continuity.

The composed macOS fixture starts an owner and Graph from an installed helper,
holds a draft, replaces the executable inode with the same candidate's signed
bytes, checks the bundle seal and exercises production unregister/register. Its
updater runs through LaunchServices as the app. A fresh relay acknowledgement
precedes editing/submitting the draft; exact owner/provider identity and Graph
progress survive. This proves installed same-build replacement, not released
version migration or rendered Desktop continuity.

At `2b9ecd9790`, eleven focused Rust transport tests and three Swift connection
tests passed; the app built headlessly. They cover newer claims, expired output,
owner unavailability, cancellation, stale actions and lost takeover replies.
Publication at `8c54f5e08a` also passed the opt-in installed replacement proof,
formatting, Clippy with warnings denied, Ruff and whitespace checks. A fixture
repair replaced `open -W`'s unreliable short-process watch with the updater's
existing completion result; it does not retry registration or replacement.
These are previous implementation evidence, not tests of this namespace cutover.

Gate/demo still own these acceptance checks after the local cutover:

- Install/update through the real selected installation; prove one registered
  server and one local endpoint. Existing paired credentials/settings survive.
  CLI help and every supervisor/client caller use the new command ownership.
- Repeat composed signed replacement with a draft and Graph live. Prove the
  equivalent Linux supervisor path; preserve normal cgroup cleanup.
- Demonstrate rendered Desktop quit/reopen, native recovery, local copy/Find/scroll
  and visible Move here. Show disabled background items and version mismatch
  truthfully. Headless model tests do not prove that interaction.
- Check CLI/socket defaults, state and attribution equivalence, subscription
  initial/change/error/cancel behavior, saturation, concurrent startup and command
  isolation. Reconnect must not duplicate provider-usage totals. Broader Xcode
  link, Linux and telemetry acceptance remains open.
- Join LOO-718's phone path to the migrated server. Lose create/start replies,
  recover one original Task or report uncertainty, and show remote route failure
  makes no local mutation. Revocation/remote disablement preserves local editing.

## Boundaries and research

Preserve LOO-720/#1809 terminal activity (integrated at `324a0f6dc4`), LOO-582/#1807
Session identity, LOO-673/#1791 Desktop queries, LOO-312/#1656 installation,
LOO-714/#1805 defaults and #1808 Wave DTO repair (Task attribution unresolved).
Recheck integration state before changing shared code. LOO-717 owns shell policy;
LOO-709/710/712 own retention, snapshots and transport. The server bridges them.

[Earlier lfd research](reports/Lessons%20from%20earlier%20lfd%20attempts.md) shows
shared executors still suffered orchestration drift (#304) and the CLI delegation
cutover remained unfinished (#825). [Installation research](reports/Machine%20service%20installation%20and%20lifecycle.md)
separates supervisor observations from product promises. These are historical
sources; their old naming and proposed timeouts do not override this design or
current owner policy. [Original Desktop scope](research_notes/desktop-server-original-scope.md)
retains the app-automation questions. The current review is
[concept-review.html](concept-review.html).
