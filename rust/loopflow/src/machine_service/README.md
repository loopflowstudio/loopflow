# Connect to the local Machine

```sh
lf machine service ensure   # obtain the selected installation's service
lf machine service status   # readiness, socket and service incarnation as JSON
```

The foreground `lf machine service run` belongs to the OS supervisor. Desktop
uses the service for Task readings, Session rename and existing terminal
attachment. Other Work operations still use their existing transport pending
LOO-718's full cutover. Desktop's UI automation socket remains app-owned.

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
  `desktop_sessions`, `machine_id`, `portfolio`, `repository_path`. Repository IDs resolve on the
  destination with `repository_path`; paths in local reads are destination paths.
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

Request start/exit diagnostics correlate service incarnation, request ID, child
PID, CLI trace ID and exit code. Ordinary CLI Process records and provider usage stay with
existing owners; observations and reconnects add no usage or Work records.
A connection error alone proves no command failure.
Service startup, command outcomes and failures use the existing `logs/lf.log`
diagnostics. Individual reads and subscription frames are not logged by default.
