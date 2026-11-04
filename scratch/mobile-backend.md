# Separate execution destinations, client access and installation

LOO-593 · Accepted November 4, 2026 · Reconciled November 4, 2026

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
[Clients, servers and execution destinations](../docs/dev/server.md).
This replaces the earlier Machine-service framing. Jack Heart now authorizes
implementation and publication of this cutover in the existing Task, followed by
interactive review. The earlier docs-only boundary is superseded. No merge or
Auto-Ship approval is given. Publication returns to the existing feature graph’s
interactive demo boundary.

## Documentation review correction · November 4

Jack Heart identified that the Machine/server/installation split appeared in
architecture docs without reaching the user guides, and requested renaming the
developer directory to `docs/dev/`. The user-facing server guide is now
`docs/server.md`, linked from setup, Ops, Surfaces, Sessions, troubleshooting and
the glossary. Developer sources, checked reference and portable export live
under `docs/dev/`; the existing `/architecture` website URLs retain their meaning.
Navigation includes the server and environment developer pages, and links from
user guides resolve to that surface. This is a local documentation correction;
it does not advance the Task past demo or authorize shipping.

## Implementation status and remaining work

The local cutover is implemented in this checkout: `lf server run/status`, hidden
installation readiness, renamed Rust/Swift protocol clients, environment flags,
DTO fixtures and packaging. Registration is under `installation/`; no serving or
setup subcommand remains on Machine. Desktop consumes initial readiness even when
it fails; subsequent Work refreshes discover the endpoint through observational
`server status`. Native Retry only reconnects. Failed-start recovery attaches
passively so a newer viewer retains its claims. The protocol wire fields remain
revision 1.

Upgrade retires old OS labels before registering the server. The native bundle
retains only an old plist migration descriptor; its obsolete argv cannot start a
new server. Custom-home launchd migration checks both disabled labels, and systemd
migration respects existing disabled/masked units. The installed revision-1 socket address/lock stay stable: changing the basename
would strand live relays running the prior executable during replacement.
One endpoint and its existing singleton lock exclude competing listeners.
No Work or credential state moves. A focused migration check found and fixed the
old launchctl parser's assumption that overrides print `true`; current macOS
prints `disabled`.

The failed-setup correction is committed at `5f140091b6`, with shared endpoint
state simplified at `4c5ea88a47`, on the cutover at `996f16fabb`; the checkout's
base remains `ac8e088f5f`. Copy/Find and pointer/scroll handlers
remain reachable while provider input is paused; rendered behavior still needs
demo evidence.

Remaining work:

- Gate the affected command, protocol, relay, Swift and packaging boundaries,
  reusing applicable focused passes. The renamed composed signed installed
  replacement passed during publication review; the broader Xcode link check
  was stopped by the host security-pressure limit and remains unproven. Formatting and Clippy already passed for this code.
- Publish the cutover to existing PR #1813 and return to interactive demo. Native
  recovery, quit/reopen, reading/copy/Find/scroll and Move here remain unverified.
  Linux supervisor replacement remains Linux-host acceptance.
- Integrate the shared contract with LOO-718 without duplicating remote admission
  or wider FFI cutover here. Combined-release acceptance remains separate.

### Related work, observed November 4

All five PRs remain open at reconciliation; none supplies landed behavior here.

| Work | Observed head | Integration boundary |
| --- | --- | --- |
| [LOO-718 / #1812](https://github.com/loopflowstudio/loopflow/pull/1812) | `983b271695` | Native iOS, remote admission, pairing and create/start; migration map below |
| [LOO-723 / #1815](https://github.com/loopflowstudio/loopflow/pull/1815) | `c7fff6b143` | Artifact selection/update; retains retired app bundles used by live processes |
| [LOO-717 / #1811](https://github.com/loopflowstudio/loopflow/pull/1811) | `b81df60579` | Machine shell lifetime and remote routing |
| [LOO-714 / #1805](https://github.com/loopflowstudio/loopflow/pull/1805) | `030ab4ef1b7` | Published installation preferences and provider defaults |
| [LOO-312 / #1656](https://github.com/loopflowstudio/loopflow/pull/1656) | `eff20d8f32` | Installation/onboarding product choices and first-run accounts |

LOO-723’s retained-bundle behavior complements server replacement: deleting an
old app is distinct from unregistering its server. The combined installation must
preserve that process-aware artifact cleanup and this branch’s post-promotion
registration hook. Its source is evidence, not a composed-release proof.
Fresh PR reads confirm the other four heads are unchanged. LOO-714's two newer
commits change configuration display and explanations; they preserve the
installation/server boundary and structured configuration output.

## Delete — do not maintain

- Removed `ServerLocation`'s separate successful-path cache and readiness flag.
  The shared lookup task retains the endpoint after success; its initial failure
  diagnosis selects observational status for later lookups. Concurrent callers
  share the same result without another installation attempt.
- Removed readiness from `Status::connect`'s retry loop and replaced
  `ServerLocation.path()`'s repeatable installation path with one initial attempt
  and observational endpoint discovery. Failed startup also discards the original
  replacing Attach intent; the existing passive recovery path preserves newer
  claims. Initial diagnostics and pane-closure cancellation remain.
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

Read-only inspection of PR head `983b271695` confirmed
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

This handoff leaves LOO-718’s checkout, schema and remote implementation with that
Task. No namespace-only wire revision or persisted credential migration is needed.

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

### Failed setup consumes readiness

Jack Heart's reconnect-only rule applies before the first successful connection.
Review at `996f16fabb` found that native Retry and reopened Work watches could
repeat installation after setup failed. The correction at `5f140091b6` consumes
readiness once per native surface or shared Work client. Later endpoint discovery
uses observational `server status`; recovery has no registration branch or second
path resolver. Concurrent waiters retain the initial diagnosis. Installation or
OS enablement repairs setup; Retry only observes the result. Pane closure still
cancels setup and recovery, and the last screen remains readable while unavailable.

The decisive failed-start proof makes registration possible after initial failure,
then verifies Retry/Work refresh does not invoke it. Independent server startup
must restore fresh Work and the exact owner on the same client, preserving newer
input/geometry claims and replaying nothing. This complements successful-start
restart and lost-takeover-acknowledgement checks; it does not replace signed
replacement or rendered acceptance. The detailed review and original diagnosis
remain at `5f140091b6:scratch/mobile-backend.md` and
`5f140091b6:scratch/concept-review.html`.

### Concept review after the failed-setup correction

Review of `4c5ea88a47` against Jack Heart's accepted direction found no further
model or interaction change and no open product choice within this local cutover.
Opening a client requests installation readiness once; reading Work or attaching
uses the server; replacing that server leaves execution with existing Session
and Graph owners. Failed setup consumes readiness too. Retry observes repaired
availability and reconnects to the exact owner; Move here alone deliberately
replaces control. These remain separate actions without a new durable viewer or
recovery record. Existing usage and skill guidance need no further redesign.

The failed-start and lost-takeover-acknowledgement counterexamples above remain
acceptance constraints. Source review confirms the corrected boundary; it adds
no runtime or rendered proof. The later publication check below supplies the
renamed installed replacement evidence; gate/demo acceptance remains below. GUI-less
distribution, before-login hosting, active-work removal and the phone takeover
gesture remain outside this accepted local scope, with no implied approval.
The review page now shows the corrected behavior rather than proposing it again.

## Evidence and remaining proof

The focused failed-setup relay proof covers terminal and byte-stream clients:
Retry leaves installation attempts unchanged while setup is possible, then an
independent server start restores the exact owner. A newer viewer retains input
and geometry; outage/read-only typing is discarded, the owner/provider identity
stays fixed, and the retained screen returns. The correction also routes a failed
first connection through passive recovery instead of the original replacing
Attach. The Swift proof covers failed subprocess and `ready: false` readiness,
read refresh and reopened watches, followed by fresh Work on the same client.

The socket contract test now owns read/write parity, watch checkpoints and trace
isolation once. Restart, installed replacement and Move here each exercise the
same smaller owner/draft/Graph continuity fixture. Packaging copies the authored
LaunchAgents directory, including the migration descriptor, without repeating
its file inventory in the release and development builders. Server fixtures now
initialize their disposable Machine store explicitly: the observational entry
correctly refused bootstrap when the previous fixture started it before any Work.

The cutover/compression evidence saved at `996f16fabb:scratch/mobile-backend.md`
records passing Rust command/protocol and five transport checks, seven Swift
connection/protocol tests, custom-home registration migration, the signed
registration harness and 22 packaging/development Python tests. Formatting,
Clippy with warnings denied, Ruff and whitespace checks passed. These establish
read/write and ViewPlan parity, changed/checkpoint/error watch behavior, command
trace isolation, observational status and owner continuity components. They do
not establish saturation/cancellation cleanup, all defaults/attribution paths,
unchanged usage totals or rendered interaction. No code changed in reconciliation.

The composed macOS fixture starts an owner and Graph from an installed helper,
holds a draft, replaces the executable inode with the same candidate's signed
bytes, checks the bundle seal and exercises production unregister/register. Its
updater runs through LaunchServices as the app. A fresh relay acknowledgement
precedes editing/submitting the draft; exact owner/provider identity and Graph
progress survive. This proves installed same-build replacement, not released
version migration or rendered Desktop continuity.

The pre-cutover evidence at `2b9ecd9790` and `8c54f5e08a` includes eleven Rust
transport checks, three Swift connection tests, a headless app build and the
opt-in installed replacement composition. The fixture’s updater completion result
replaced an unreliable `open -W` short-process watch; no registration retry was
added. Those earlier passes do not validate the renamed installation path; the
publication rerun below does. Demo owns rendered continuity.

Gate/demo still own these acceptance checks after the local cutover:

- Install/update through the real selected installation; prove one registered
  server and one local endpoint. LOO-718’s paired credentials/settings must survive
  the combined integration; this local branch has none to migrate.
  CLI help and every supervisor/client caller use the new command ownership.
- Prove the equivalent Linux supervisor replacement path with a draft and Graph
  live; preserve normal cgroup cleanup. The renamed macOS composition passed.
- Demonstrate rendered Desktop quit/reopen, native recovery, local copy/Find/scroll
  and visible Move here. Show disabled background items and version mismatch
  truthfully. Headless model tests do not prove that interaction.
- Extend the existing parity/checkpoint evidence to defaults/attribution,
  cancellation cleanup, saturation and concurrent startup. Reconnect must not
  duplicate provider-usage totals. Broader Xcode link and telemetry acceptance
  remain open.
- Join LOO-718's phone path to the migrated server. Lose create/start replies,
  recover one original Task or report uncertainty, and show remote route failure
  makes no local mutation. Revocation/remote disablement preserves local editing.

## Boundaries and research

Preserve LOO-720/#1809 terminal activity (integrated at `324a0f6dc4`), LOO-582/#1807
Session identity, LOO-673/#1791 Desktop queries, LOO-312/#1656 installation,
LOO-714/#1805 defaults and #1808 Wave DTO repair (Task attribution unresolved).
LOO-717 owns shell policy; LOO-709/710/712 own retention, snapshots and transport. The server bridges them.

[Earlier lfd research](reports/Lessons%20from%20earlier%20lfd%20attempts.md) shows
shared executors still suffered orchestration drift (#304) and the CLI delegation
cutover remained unfinished (#825). [Installation research](reports/Machine%20service%20installation%20and%20lifecycle.md)
separates supervisor observations from product promises. These are historical
sources; their old naming and proposed timeouts do not override this design or
current owner policy. [Original Desktop scope](research_notes/desktop-server-original-scope.md)
retains the app-automation questions. The current review is
[concept-review.html](concept-review.html).

Check: `git diff --check` passed; saved `cargo test -p loopflow --test transport_tests server_failed_setup_retry_only_reconnects_and_preserves_newer_claims` at `5f140091b6` and `scripts/test_desktop.sh --filter ServerClientTests --jobs 4 -Xswiftc -gnone` after compression at `4c5ea88a47` passed and remain applicable; the publication rerun below covers composed installed replacement, gate owns broader checks and demo owns rendered acceptance.

## Publication verification (November 4)

At `61389c3007`, the renamed
`installed_server_preserves_owner_and_graph_across_replacement` passed explicitly
outside the network sandbox. The signed installed helper retained the exact
owner/provider, held draft and running Graph through replacement, a fresh relay
acknowledgement, draft submission and Graph progress. This remains same-build
replacement, not released-version migration or rendered acceptance.

The publication check found two architecture-map errors: the server row repeated
the CLI binary owner, and installation omitted the absolute launchctl edge. The
map now passes all seven coverage categories after correcting those descriptions.
An accidentally broad gate invocation was stopped during its Python build phase;
it is not a gate pass. Existing focused implementation passes remain applicable.
Publication is for the interactive demo; the later delivery gate and native/Linux
acceptance remain required. No merge or Auto-Ship approval is given.

The focused publication run (`scripts/test.py --base HEAD --loopflow
--reuse-passing`, with only documentation edits uncommitted) passed architecture
and all 72 website checks. Website reference generation used the installed CLI,
so that pass does not validate the candidate command tree. Xcode generation passed;
`build-for-testing` was interrupted after 94 seconds when `syspolicyd` held 257%
CPU for three samples. The runner classified this as host security pressure,
not a compiler failure. No checkout Xcode compilers remained after cleanup.
Xcode app/test-runner linking remains unproven and belongs to delivery gate before
merge. Evidence: `.lf/tmp/gate/run-86568/loopflow/xcodebuild.log`. The host guard
was not bypassed. Publication proceeds to Jack Heart's authorized interactive
demo boundary, with this limit stated in the PR.
