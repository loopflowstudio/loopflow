# Separate execution destinations, client access and installation

LOO-593 · Design revised November 4, 2026 · Interactive demo

## Accepted direction

Jack Heart requested returning to the drawing board after identifying that
`lf machine service` mixes remote execution management with client API access.
Jack selected a documentation/design rewrite around this split:

- **Machine** names where work runs: identity, SSH connections, destination
  defaults, pools and rentals. Keep `lf machine` and `lf --machine` here.
- **Server** gives Desktop and mobile access to Loopflow on a host. Move client
  admission, availability and the serving entry point to `lf server`.
- **Installation** selects and updates artifacts and registers the server.
  Keep that lifecycle under `lf install`, including automatic local setup.
- **Desktop** owns its windows, panes, rendering and app-owned automation API.
  Keep `lf desktop` and `DesktopServer` separate from the shared server.

The canonical boundary and proposed usage are in
[Clients, servers and execution destinations](../docs/architecture/server.md).
This replaces the earlier Machine-service framing. The command migration is
not implemented by this review. Jack requested docs edits, not a Graph restart,
publication or shipping. Earlier authorization reached interactive demo; no
merge or Auto-Ship approval follows from this rewrite.

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

## What exists and what must change

LOO-593 has implemented a local server under `machine_service/`, with roadmap
read/watch, Session rename through the CLI and exact existing-owner attachment.
Those migrated operations have no direct-FFI fallback. Other Work operations
still use their existing adapters. App-owned UI automation remains separate.

LOO-718/PR #1812 incorporates `07a6a184d4`; inspection at `983b271695` on November 4
found remote HTTPS/WSS over host Tailscale, pairing/revocation, native iOS and
recoverable create/start work. That is evidence from the parallel branch, not
verification of the combined release. LOO-593 is the local foundation; LOO-718
owns the remote/mobile experience. Both need the accepted naming boundary.

| Current implementation | Target responsibility | Required change |
| --- | --- | --- |
| `lf machine service run/status` | Server availability | Move to proposed `lf server run/status`; status stays observational |
| `lf machine service ensure` and registration under `machine_service/` | Installation plus connection readiness | Put selected-build registration/update under installation; keep connection/bootstrap in the client; remove public setup machinery |
| Remote enablement, pairing and device commands in LOO-718's Machine group | Server client access | Move under the server group; settle the exact subcommands with that branch |
| `MachineService`, `MachineProtocol`, service-specific `machine_*` files | Server client/protocol | Rename callers and fixtures together; keep domain Machine identity intact |
| Bundled LaunchAgent/systemd names and persisted access settings | Installation migration | Replace registration once; preserve settings/credentials and prevent duplicate listeners |
| Remaining Work FFI adapters | Shared operations reached through server | Inventory and delete each replaced route during the wider cutover; preserve renderer FFI |

Do not create compatibility aliases for internal commands or duplicate protocol
implementations. Persisted state and installed supervisor registrations need an
explicit migration. Do not rename `MachineId`, `machine_id`, execution-location
records, `LF_HOME` or ordinary SSH routing just because the server reports them.
The namespace change alone does not require a new wire revision.

## Preserve the working contract

[The local protocol README](../rust/loopflow/src/machine_service/README.md) owns
revision-1 operations and bounds. Rust wire types, the Swift mirror and fixtures
under `tests/fixtures/dto/machine/` currently supply both clients. `view_plan`
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
These are previous implementation evidence, not tests of the proposed rename.

Next proof after the coordinated cutover:

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
