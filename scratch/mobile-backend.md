# Separate execution destinations, client access and installation

LOO-593 · Deletion pursuit selected November 4, 2026

## Current pursuit: shared direct writes and the Desktop FFI deletion

Jack Heart selected [direct server writes](server-write-direction.md) after the
full-CLI cutover was implemented. That direction supersedes the subprocess
execution design below and in older evidence. Keep the live feature Graph and
PR #1813; its authored loop must return to implement before publication/demo.
No merge, Auto-Ship or Task completion is authorized.

The unpublished deletion in this checkout removes the embedded Work library.
Preserve it. Every native shared Read, Write and Subscription uses the local
server; Ghostty rendering, app-owned automation, ordinary execution launches,
remote streams and existing Session/Graph owners remain separate.

### Delete — do not maintain

| Concrete removal | Surviving path / remaining cut |
| --- | --- |
| `rust/loopflow-ffi/`, its C ABI and Cargo entries; `LoopflowLibrary.swift`, `LoopflowFFI/module.modulemap`, `LoopflowLibraryTests.swift` | Already removed. Keep shared Rust operations behind `ServerClient`; no Swift FFI fallback. |
| Static Loopflow-library linking and FFI-only packaging/build steps | Already removed from SwiftPM, Xcode, CI and scripts. Keep Ghostty. |
| `serve_in_app`, `open_in_app`, `with_call_process`, `Write::perform` and their exclusive tests | Already removed. Do not restore request-sized Process records or the duplicate mutation executor. |
| `server/service.rs::command` subprocess adapter and `read_command_output` | **Next implementation cut:** direct typed writes through one shared CLI/server mutation path, with explicit request context and output. |
| Duplicated mutation/default/output branches that shared execution replaces in `bin/lf.rs` and `lf/commands/` | Remove in the same cut. Preserve complete routing, defaults, errors and attribution; never redirect global cwd/env/stdin/stdout. |
| CLI-child Process assertions exclusive to the old server adapter | Replace with request trace/effect correlation and real OS identity assertions. Keep effect parity, failure, >64 KiB save, cancellation and concurrent-write coverage. |

### Remaining implementation and proof

The next implement pass owns the shared mutation path and request-scoped
telemetry specified in `server-write-direction.md`. Current subprocess results
are historical evidence, not acceptance of direct writes. Arrangement still
creates no Process. Distinct concurrent requests must not borrow the server
launcher's Session/Task or create fictitious OS processes. A lost reply remains
outcome unknown; cancellation releases the viewer without replaying a write.

Follow the complete mutation paths, not just `Write`'s enum match. Existing
`bin/lf.rs` dispatch owns routing/default resolution and prints diagnostics;
`journal::current_context` selects process-global context in a CLI;
`session::prepared`, `work_route::forward`, and `config::with_agent_override`
are relevant existing capabilities. Sharing lower-level operations alone must
not leave two orchestration implementations. The prior lfd research documents
that failure mode.

Gate owns affected correctness and packaging checks after implementation; sync
owns PR #1813's base conflicts without resurrecting FFI. The contributing
conversation owns the native demo and matched benchmark on the completed build:
create a Task, open its conversation, rename and save, verifying UI and persisted
state. Leave precise build/test evidence here; no rendering environment is
available in this Graph step. Publication must describe the final implementation
and its remaining platform limits, then stop at interactive demo.

Retain [native evidence](server-cutover-demo.md),
[raw measurements and baseline harness](server-performance/results.md), and
[contribution/build handoff](server-cutover-handoff.md). The earlier demo predates
the pane-close repair and direct writes. Matched old warm medians were read
21.06 → 26.79 ms and rename 50.68 → 87.60 ms. Remeasure the same workload after
removing CLI startup; no matched embedded RSS sample supports memory savings.
Prior namespace-only detail remains at `8d6c7ee75b:scratch/mobile-backend.md`.
The pre-compression uncommitted plan is retained at
`/tmp/loo593-compress-original-mobile-backend.md`.

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

## Retained installation and documentation decisions

Jack Heart selected `docs/dev/` for developer sources; existing `/architecture`
website URLs stay stable. The user-facing server guide and links from setup,
Ops, Surfaces, Sessions, troubleshooting and glossary remain current.

Namespace cutover `996f16fabb` and readiness repairs `5f140091b6` / `4c5ea88a47`
established `lf server run/status` and installation-owned readiness. Keep the
revision-1 socket/lock for running relays and the legacy plist solely to inspect
and retire installed registration while preserving disabled choices. These
persisted-installation migrations are not obsolete embedded Work code.

## Experience and ownership

Install Loopflow, open Desktop and edit a Session. Local client access is prepared
by installation; no server setup ritual or Tailscale is needed. Enable remote
access separately to pair a phone with that host. A phone is a client of a Machine,
not another execution destination. Selecting another worker remains an explicit
Machine operation.

```text
Desktop ─ private Unix socket ─┐
                             ├─ server ─ shared Rust Read / Subscription
phone ─ authenticated API ────┘         ├ shared typed mutation + scoped request telemetry
        (LOO-718)                      └ existing Session owner ─ agent / PTY

installation ─ selected executable + server registration / replacement
Machine routing ─ SSH destination's ordinary lf invocation
Desktop automation ─ app-owned windows / panes / menus
```

The server handles admission, framing and bounded delivery. Shared operations retain defaults, mutation, attribution and recovery. Session owners
retain provider processes, input/geometry claims and lifetime policy. Graphs
retain their own execution. Drafts, active turns, pending requests and running
Graphs already protect their owners; the server adds no retention authority.
CLI use does not require the server. No scheduler, shadow Work store or durable
viewer registry belongs here.

## Local contract and mobile integration

LOO-593 implements the local server under `server/`. Every shared Read, Write and
Subscription now crosses it, along with exact existing-owner attachment. The
Loopflow FFI crate, Swift adapter and static-link build paths are deleted. Generic
execution launches and remote streams retain the CLI; app-owned UI automation and
Ghostty rendering remain separate. See server-performance/results.md and server-cutover-demo.md for the
subprocess-era evidence; direct-write acceptance is still pending.

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
protocol references with this foundation. Framing now uses one `MAX_FRAME` limit;
remove `MAX_REQUEST` imports in the mobile adapter. Preserve every wire field including
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
Closing cancels; reconnect does not replay store revisions. Commands must reuse the complete shared mutation path with explicit request
context and fresh traces; the subprocess adapter is pending removal. Request IDs correlate;
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

## Acceptance limits

Preserve explicit limits for native quit/reopen and rendered copy/Find/scroll/Move
here; Linux supervisor replacement under normal cgroup cleanup; Xcode packaging;
subscription saturation/cancellation; defaults, attribution and provider usage
beyond tested operations; and combined mobile integration. Remote create/start
lost-reply recovery belongs to LOO-718. No numeric performance target was selected.
Signed same-build replacement evidence from the namespace-only build remains at
`8d6c7ee75b:scratch/mobile-backend.md`, not proof of this final implementation.

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

Compression keeps one frame-size policy, surfaces watch errors only on the
stream, and reuses `LocalServerFixture` for recovery tests. The `docs/dev/` move
also removes the obsolete reference-page special cases and rendering flag.

Check: `scripts/test_desktop.sh --filter 'ServerClientTests|WorkObservationTransportTests'` passed 11 tests; `cargo test -p loopflow --lib server::` passed 3; portable-doc pytest and route/link checks, Ruff, fmt and diff checks passed (`/tmp/loo593-compress-{swift,rust}.log`). Direct writes remain implement's next cut; gate owns affected acceptance, the contributing conversation owns the new native demo/benchmark.
