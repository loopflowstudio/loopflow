# Separate execution destinations, client access and installation

LOO-593 · Accepted direction and implementation reconciled November 4, 2026

## Current pursuit: shared direct writes and the Desktop FFI deletion

Jack Heart selected [direct server writes](server-write-direction.md) after the
full-CLI cutover was implemented. Accepted scope: one complete Rust mutation path
for CLI and server, request-scoped telemetry, and deletion of the embedded Work
library. Publication of PR #1813 and interactive demo are authorized; merge,
Auto-Ship and Task completion are not. The existing feature Graph owns delivery.

### Implemented at `59287ba3ed`

Every shared native `Read`, `Write` and `Subscription` uses the local server.
`rust/loopflow-ffi/`, its C ABI/Cargo entries, `LoopflowLibrary.swift`, the Swift
module map and FFI-only tests/linking/packaging are removed. `serve_in_app`,
`open_in_app`, `with_call_process` and the duplicate `Write::perform` executor
are also gone. Ghostty rendering, app-owned automation, ordinary execution
launches, remote streams and existing Session/Graph owners retain their roles.

At that commit, served writes spawned the full CLI. The working direct-write
cut replaces that adapter with shared mutation dispatch.

Desktop Task creation still uses `RegistryQuery.createTask` → ordinary
`task create --run --json`; it is an execution launch, not a `Write` variant.
The required native create/open/rename/save demo spans both that launch path and
served mutations. Removing ordinary served-write subprocesses does not move
provider or Graph lifetime into the server.

Main's #1815 (`dfd454cfc7`) is integrated by merge `59287ba3ed`, after checkpoint
`ced68911ee`. The earlier base conflict is resolved locally; publication remains.
Its coherent app/helper selection and custom-location update behavior must survive
further changes. Infrastructure's November 4 memory also records reported lost
input/crashes after upgrades (LOO-724); liveness alone is not continuity evidence.
No cause or combined-release success follows from the merge.

### Shared direct-write implementation

`lf/commands/mutation.rs` owns routing, selection, effects and formatting for
served operations and their CLI commands. It accepts a directory and streams;
`Write` retains the wire shape and CLI spelling. The server uses a blocking
worker, with no ordinary CLI child. `journal::request` masks the launcher's
Process/caller and carries diagnostics, agent choice and tracing into nested
blocking operations without changing global cwd/env/stdio. Requests never
create Process rows; existing provider and Graph owners remain unchanged.

Jack Heart's November 4 review selected the existing `logs/lf.log` path. Request
spans/effects use its enabled server target, carry trace and target, and record
duration/result/panics. No extra request-log directory remains. Ship-stop records
its existing timestamp without an OS Process. The Task's single migration adds
`ship_stopped_person`, preserving the person at admission rather than inferring
historical authorship from later Machine settings; older unknown names stay unknown.

### Delete — do not maintain

- `server/service.rs`'s CLI spawn and `read_command_output`: removed; preserve
  framing limits, output/error/exit status, no replay and unknown outcomes.
- Task/Wave mutation branches in `bin/lf.rs`, Session rename/bind orchestration,
  and Desktop saved-write branches: moved to shared command handling. Preserve
  CLI flags/defaults and app-owned automation.
- Process-count equivalence in `served_reads`: replaced with effect equivalence
  and proof that requests create no Process rows; preserve every wire variant,
  large saves, and arrangement's no-Process behavior.
- Redundant Session/Desktop mutation re-entry and `portfolio::run`: removed;
  CLI dispatch already selects the shared executor before reaching those handlers.
  Request entry and nested workers use the same unwind-safe scope restoration.
- Already removed Desktop FFI crate/adapter/module-map/linking/packaging stay
  deleted. Provider launches and remote streams retain their existing owners.

Focused parity covers every wire variant, >64 KiB saves, failed writes, normal
request logs, concurrent traces, no fabricated Processes and admitted lost replies.
Remote ownership verifies no local fallback. The final audit also carries the
request directory into native-conversation adoption and remote source resolution,
keeps pending-sync diagnostics in the request, and avoids waiting for terminal
EOF when a remote CLI mutation takes no body. Both final focused reruns passed;
no product judgment is missing.

### Remaining proof and delivery

Gate owns affected correctness/packaging checks on the completed implementation,
including full Desktop and bounded Xcode builds. The existing Graph owns sync,
checkpoint and publication of PR #1813, stopping at interactive demo. No merge,
Auto-Ship or Task completion is authorized.

The [direct-write demo](direct-write-demo.md) proves native Task creation,
conversation, rename, file save and description save with persisted-state receipts.
The contributing conversation's bundle predates the final cwd/error/stop repairs;
focused checks cover those changes, not a fabricated final-build screenshot claim.
Its attempted autonomous research Graph lacked a managed Codex account; native
conversation worked. This is operator evidence, not Jack Heart's acceptance.

[Matched measurements](server-performance/results.md) retain raw samples:
subprocess → direct rename median/p95 87.38/105.30 → 10.77/20.63 ms; read
26.94/34.42 → 28.00/52.67 ms. The rename workload improved, while read tail latency
and the single restart observation worsened. Host/Graph work was uncontrolled;
there is no general read, startup or memory-savings claim. The earlier embedded
baseline is historical. `scratch/demo.html` holds the prepared review artifact.

The [handoff](server-cutover-handoff.md) preserves earlier failures: the full
Desktop run failed Portfolio setup and stalled during Work observation shutdown;
focused repairs passed, but no full-suite pass follows. Earlier server library
checks emitted an Apple compact-unwind linker warning. Namespace-only signed
replacement and the [earlier native demo](server-cutover-demo.md) retain their
limits. Pre-realignment detail is committed at `59287ba3ed`.

## Accepted responsibilities and shared contract

Jack Heart selected the November 4 split after reviewing mixed client-access and
execution-management responsibilities. Machine owns execution destinations,
identity, SSH, defaults, pools and rentals. Server owns shared client access.
Installation owns artifact selection, updates and automatic local registration.
Desktop owns windows, panes, Ghostty rendering and its automation API. CLI use
stays independent of server availability; no scheduler, shadow Work store,
viewer registry or new execution-lifetime owner belongs in this service.

The canonical boundary and user commands are in
[Clients, servers and execution destinations](../docs/dev/server.md).
Developer sources moved to `docs/dev/` at Jack's request; existing `/architecture`
website URLs remain stable. `lf server run/status` replaces the former Machine
service commands. Installation's hidden readiness bridge and legacy plist only
support installed registration migration. Revision-1 socket/lock, Machine IDs,
execution-location records, `LF_HOME` and wire fields retain their identities.
Keep disabled background items disabled. No internal command aliases or duplicate
protocol paths are required.

All shared native Read/Write/Subscription operations use the local server;
ordinary execution launches and remote streams retain their existing owners.
Local setup is automatic and needs no Tailscale. Remote access is separately
opt-in. A phone is a client of a Machine; selecting a worker is explicit.
LOO-718 owns remote admission and iOS. Read-only inspection of its PR #1812 at
`983b271695` found HTTPS/WSS over host Tailscale, pairing/revocation, native iOS,
and recoverable create/start; this does not prove the combined release.

Earlier namespace mappings, decisions and protocol diagrams are preserved at
`59287ba3ed:scratch/mobile-backend.md`; current source/API documentation replaces
the duplicate inventory here.

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
Closing cancels; reconnect does not replay store revisions. Commands use shared mutation dispatch with explicit directory/I/O and scoped
request traces. Request IDs correlate;
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

Readiness is consumed once per native surface/shared Work client, including
failed first setup. Retry and reopened watches observe `server status`; neither
re-registers nor chooses another build. Concurrent waiters retain the diagnosis.
The proof makes registration possible after failure, checks that Retry does not
invoke it, then starts the server independently: the same client recovers fresh
Work and the exact owner without replacing newer claims or replaying input.
Closure cancels recovery and the last screen stays readable. This supplements,
but never substitutes for, signed replacement and rendered acceptance. Original
review and diagnosis: `5f140091b6:scratch/mobile-backend.md` and that commit's
`scratch/concept-review.html`.

## Acceptance limits

Preserve explicit limits for native quit/reopen and rendered copy/Find/scroll/Move
here; Linux supervisor replacement under normal cgroup cleanup; Xcode packaging;
subscription saturation/cancellation; defaults, attribution and provider usage
beyond tested operations; and combined mobile integration. Remote create/start
lost-reply recovery belongs to LOO-718. No numeric performance target was selected.
Signed same-build replacement evidence from the namespace-only build remains at
`8d6c7ee75b:scratch/mobile-backend.md`, not proof of this final implementation.

## Boundaries and research

Preserve LOO-723/#1815 installation coherence (integrated at `59287ba3ed`),
LOO-720/#1809 terminal activity (integrated at `324a0f6dc4`), LOO-582/#1807
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

Check (November 4): `cargo test -p loopflow --lib journal::request::` (2 passed) and `cargo test -p loopflow --test served_reads` (1 passed) verify the dispatcher/scope simplification (`/tmp/loo593-compress-{request,parity}.log`); `cargo fmt` and `git diff --check` passed. Gate retains lint and full consumer checks. Earlier verification: `cargo test -p loopflow --test served_reads` and `cargo test -p loopflow --test integration machine_commands::served_ownership_routes_to_its_writer_and_never_falls_back_locally` passed (`/tmp/loo593-direct-{parity,routing}-final.log`); `scripts/check_migrations.py`, `cargo fmt` and `git diff --check` passed. Earlier request-scope/stop tests remain applicable (`/tmp/loo593-{request,stop}.log`). The new cwd test first failed only on macOS path spelling; canonical-path comparison passed. Gate owns clippy, full affected suites, Desktop and Xcode; publication remains the Graph's later step.
