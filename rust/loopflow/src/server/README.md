# Connect to the local Loopflow server

```sh
lf server status   # readiness, socket and server incarnation as JSON
```

Installation prepares client access automatically. Desktop's first connection
requests readiness through the selected installation; subsequent connections and
terminal recovery never re-register a server. `lf server status` only observes.

The foreground `lf server run` belongs to the OS supervisor. Desktop
uses the service for all shared Work reads, writes and subscriptions, plus
existing terminal attachment. It links no embedded Loopflow library. Desktop's UI automation socket remains app-owned.

`installation/server.rs` owns registration and replacement. Its hidden
`install server` bridge is for local clients, not a separate setup workflow.

On macOS the signed selected app bundles `com.loopflow.server.plist` and
registers it with SMAppService. Explicit development Machines use separately
named launchd jobs. Linux uses a user systemd service. Closing Desktop does not
stop the service. Session owners retain their existing lifetime policy; this
service creates no PTYs, retention records or Graphs. Logout/reboot do not
preserve live processes. Disabled Background Items remain disabled. Upgrade
retires the previous
`com.loopflow.machine` registration before starting `com.loopflow.server`.
The old bundled plist remains only for SMAppService approval inspection/removal.
Development jobs and Linux units migrate their hashed labels the same way;
existing disabled/masked units require explicit enablement. The revision-1
`machine.sock` address and its lock remain stable so already-running relays can
recover across the rename. There is one listener, with no alias or alternate
protocol path. Work and credentials stay in place.

## Local protocol, revision 1

One operation per same-user Unix connection. Every control message is a four-byte
big-endian length followed by UTF-8 JSON. Requests and responses are capped at 32 MiB and connections at 64. Writes have a five-second deadline. The socket
and its directory are private; remote proxies must never forward this endpoint.
Remote admission is outside this revision.

`wire.rs` owns the Rust types; `swift/Loopflow/Services/ServerProtocol.swift`
mirrors them. Executable fixtures live in `tests/fixtures/dto/server/`.

A request has required `version`, `id` and `operation`. A response has required
`id`, `instance` and `result`. Unsupported revisions fail before reading Work.
The bootstrap result reports the Machine, service build and executable SHA-256,
revision, limits and capabilities. Each restart changes `instance`. Default-Machine
setup delegates to the promoted installation; source builds use an explicit
`LF_HOME` to keep experiments separate.

- `read {query}` uses the existing `Read` shape. Every shared read is supported. Repository IDs resolve on the
  destination with `repository_path`; paths in local reads are destination paths.
  `view_plan` returns the existing WorkViewPlan, including the recorded primary
  Session; roadmap membership alone cannot select one. A missing primary stays
  absent. Resolve its owner with `session_owner` before attachment.
- `subscribe {query,generation}` supports every shared read with a watch. `reading {generation,frame}`
  wraps the existing complete `answers/unavailable/body` watch frame. Subsequent
  client frames are existing watch checkpoints (`{"id":3}`) or argument changes.
  The shared watch validates them, including its 16 KiB limit; an invalid
  checkpoint ends the watch with an `unavailable` reading.
  Closing cancels. Reconnect opens a fresh subscription; no replay cursor exists.
- `command {change}` accepts the shared `Write` shape. The server calls the same
  Rust mutation dispatcher as the CLI, with the request's directory and streams.
  `command {trace_id,output,stderr,code}` returns its output and exit status.
  Ordinary writes start no CLI child. Machine routing still uses SSH and never
  falls back to local execution. Requests create no Process rows. A lost response means
  outcome unknown; inspect state before retrying. IDs correlate diagnostics and
  are **not** durable deduplication receipts. No automatic resubmission.
  Canceling a native request closes its connection and releases the client's wait;
  a command already dispatched may still complete. Cancellation never replays it.
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

Request diagnostics correlate the response trace with its target, duration,
result, service incarnation and request ID. The existing log formatter adds the
real server PID and executable identity. Request spans and nested effect events
are enabled in normal `logs/lf.log` output; input bodies are omitted. Panic
unwinding logs an unknown outcome and restores the worker's scope.

`journal::request` isolates caller attribution and propagates it into nested
blocking operations. It never changes process cwd, environment or standard
streams. CLI Process records and provider usage stay with existing owners;
arrangement, observations and reconnects add no Process or usage records.
A connection error alone proves no command failure.
Service startup, command outcomes and failures use the existing `logs/lf.log`
diagnostics. Individual reads and subscription frames are not logged by default.
