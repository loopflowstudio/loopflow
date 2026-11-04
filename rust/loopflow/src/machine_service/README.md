# Connect to the local Loopflow server

This README documents the **current implementation**. The accepted
[responsibility split](../../../../docs/architecture/server.md) moves serving and
client access to `lf server`, with registration/update owned by installation.
That command and source-name cutover is pending; the commands below still use the
current `machine service` spelling. Machine connections, pools, rentals and SSH
routing remain separate.

```sh
lf machine service ensure   # obtain the selected installation's service
lf machine service status   # readiness, socket and service incarnation as JSON
```

The foreground `lf machine service run` belongs to the OS supervisor. Desktop
uses the service for Task readings, Session rename and existing terminal
attachment. Other Work operations still use their existing transport pending
LOO-718's full cutover. Desktop's UI automation socket remains app-owned.

Installation owns registration and replacement, even while its adapter lives in
this module. Readiness must not become a second installation authority.

On macOS the signed selected app bundles `com.loopflow.machine.plist` and
registers it with SMAppService. Explicit development Machines use separately
named launchd jobs. Linux uses a user systemd service. Closing Desktop does not
stop the service. Session owners retain their existing lifetime policy; this
service creates no PTYs, retention records or Graphs. Logout/reboot do not
preserve live processes. Disabled Background Items remain disabled.

## Local protocol, revision 1

One operation per same-user Unix connection. Every control message is a four-byte
big-endian length followed by UTF-8 JSON. Requests are capped at 64 KiB, responses
at 32 MiB and connections at 64. Writes have a five-second deadline. The socket
and its directory are private; remote proxies must never forward this endpoint.
Remote admission is outside this revision.

`wire.rs` owns the Rust types; `swift/Loopflow/Services/MachineProtocol.swift`
mirrors them. Executable fixtures live in `tests/fixtures/dto/machine/`.

A request has required `version`, `id` and `operation`. A response has required
`id`, `instance` and `result`. Unsupported revisions fail before reading Work.
The bootstrap result reports the Machine, service build and executable SHA-256,
revision, limits and capabilities. Each restart changes `instance`. Default-Machine
setup delegates to the promoted installation; source builds use an explicit
`LF_HOME` to keep experiments separate.

- `read {query}` uses the existing `Read` shape. Supported reads: `roadmap`,
  `desktop_sessions`, `machine_id`, `portfolio`, `repository_path`, `view_plan`. Repository IDs resolve on the
  destination with `repository_path`; paths in local reads are destination paths.
  `view_plan` returns the existing WorkViewPlan, including the recorded primary
  Session; roadmap membership alone cannot select one. A missing primary stays
  absent. Resolve its owner with `session_owner` before attachment.
- `subscribe {query,generation}` supports `roadmap`. `reading {generation,frame}`
  wraps the existing complete `answers/unavailable/body` watch frame. Subsequent
  client frames are existing watch checkpoints (`{"id":3}`) or argument changes.
  The shared watch validates them, including its 16 KiB limit; an invalid
  checkpoint ends the watch with an `unavailable` reading.
  Closing cancels. Reconnect opens a fresh subscription; no replay cursor exists.
- `command {change}` supports `session_rename` using the existing `Write` shape
  and argv mapping. `command {trace_id,output,stderr,code}` reports the real CLI result.
  The CLI owns mutation, defaults and Process attribution. A lost response means
  outcome unknown; inspect state before retrying. IDs correlate diagnostics and
  are **not** durable deduplication receipts. No automatic resubmission.
- `session_owner {session}` resolves a Session ID to `session_owner {owner}`.
  A null owner means no recorded transport; resolution never starts a provider.
  An owner can finish between lookup and attachment, which then fails explicitly.
- `attach {owner}` takes the exact existing owner Process ID. `bridged` changes
  this connection to the unchanged binary owner protocol in
  `transport/README.md` and `transport/wire.rs`. Each terminal has its own socket.
  The owner alone grants input/geometry, handles bounded output continuation and
  revokes stale control. No automatic owner replacement or raw-input replay.
  Desktop automatically reconnects the exact owner with bounded backoff, keeping
  connection status in native chrome outside terminal bytes. Recovery uses Resume
  for byte streams or a fresh terminal screen and requests free claims without
  replacement. Newer viewers retain input and geometry. Unavailable keeps the last
  screen marked stale; Retry reconnects without preparing or resuming a provider.
  A connection failure alone never means the Session ended.

Request start/exit diagnostics correlate service incarnation, request ID, child
PID, CLI trace ID and exit code. Ordinary CLI Process records and provider usage stay with
existing owners; observations and reconnects add no usage or Work records.
A connection error alone proves no command failure.
Service startup, command outcomes and failures use the existing `logs/lf.log`
diagnostics. Individual reads and subscription frames are not logged by default.
