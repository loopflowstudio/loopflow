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
Remote callers use the separate device admission below.

## Pair a phone through host Tailscale

```sh
lf machine service run --remote-port 49250
# In a second terminal, after choosing an unused HTTPS port:
tailscale serve --bg --https=8443 http://127.0.0.1:49250
lf machine service pair --endpoint https://machine.tailnet.ts.net:8443 \
  --name 'My iPhone' --output /private/tmp/loopflow-phone.json
lf machine service devices
lf machine service revoke DEVICE_ID
```

The foreground service also listens on loopback when `--remote-port` is supplied.
Host Tailscale Serve supplies tailnet-only HTTPS; never use Funnel. Inspect existing
Serve routes before choosing a port. This first carrier does not change Serve
configuration or persist the remote-port setting in the installed service yet.

The private setup file contains a one-use ticket valid for ten minutes. It is
credential material: transfer it directly to the intended phone, never commit it
or paste it into logs. A phone exchanges it at `/v1/pair` for a device credential;
the native adapter must keep that credential in Keychain. Only hashes are stored in the Machine's
SQLite database. A lost pairing reply requires a new ticket; it never reissues the
same credential. Device listings contain IDs and names, not credentials. Revoking
a device rejects new connections and terminates existing ones on their next
authorization check (every five seconds; failed checks also close the connection).

Both routes use native WebSockets with `Authorization: Bearer …`. `/v1/pair`
returns one binary JSON `{version,device,token}` message. `/v1/machine` carries the
unchanged length-prefixed Machine stream, followed by owner frames after `bridged`.
WebSocket message boundaries have no application meaning. Binary chunks are capped
at 64 KiB and copy buffers are bounded; slow readers lose their connection without
stopping execution. Browser origins and credentials in URLs are rejected.

The remote carrier checks its own device grant before shared operation dispatch.
It never forwards to the private Unix socket or trusts a reverse proxy's UID or
identity headers. The same connection capacity covers both carriers.
The native iOS adapter imports/pastes this setup file and stores its grant in
Keychain. Pairing and Work reads have passed through actual host Tailscale in
Simulator. The opt-in native acceptance also exercises a real retained owner,
editing, competing viewers, passive reconnect and one exact submission without
provider relaunch. Manual Desktop and separate physical-phone acceptance remain
open. [Run the native proof](../../../../swift/LoopflowiOS/README.md#opt-in-host-tailscale-acceptance).

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
  Desktop retains its relay after bridge loss: Ctrl+R reconnects the exact owner,
  Ctrl+] closes. Recovery uses Resume for byte streams or a fresh terminal screen,
  requests free claims without replacement, and reports an unavailable owner
  without running provider resume. Newer viewers retain input and geometry.

Request start/exit diagnostics correlate service incarnation, request ID, child
PID, CLI trace ID and exit code. Ordinary CLI Process records and provider usage stay with
existing owners; observations and reconnects add no usage or Work records.
A connection error alone proves no command failure.
Service startup, command outcomes and failures use the existing `logs/lf.log`
diagnostics. Individual reads and subscription frames are not logged by default.
