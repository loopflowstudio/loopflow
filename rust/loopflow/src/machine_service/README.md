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
lf machine service pair --name 'My iPhone' --output /private/tmp/loopflow-phone.json
lf machine service devices
lf machine service mobile --status
lf machine service revoke DEVICE_ID
lf machine service mobile --off
```

Sign in to host Tailscale first. Pairing finds a working CLI, derives this Machine’s
DNS name, obtains the existing Machine service and configures an unused private
HTTPS port. It enables the loopback carrier in place, without restarting the local
service or its Desktop connections. Import or paste the generated file into iOS.
`lf machine service mobile` enables the same endpoint without creating a ticket.

Setup preserves other background and foreground Serve routes. It reuses only the
complete private route recorded for this Machine; an edited or Funnel-enabled
former route is left alone and the carrier moves to a new loopback port. It never
runs Funnel or resets Serve. Private reachability alone never grants Loopflow
access; the phone still needs its separate device grant.

The enabled setting, endpoint and loopback port live in this Machine’s config.yaml.
The existing supervisor starts the same service, which restores that listener.
Failure to bind the mobile port leaves the local service running. `mobile --off`
stops remote admission and existing remote connections, preserves local viewers,
and records disabled state before cleanup. It removes only its unchanged owned
Serve route. If Tailscale is unavailable, the disabled record preserves route
ownership for a later cleanup retry; a service restart does not re-enable it.
Device grants remain revocable independently of endpoint enablement.

For isolated integration or an already configured host, `run --remote-port PORT`
and `pair --endpoint HTTPS_ORIGIN` remain explicit manual controls. Normal pairing
requires neither. Desktop Settings → Mobile calls the same setup operation and
shows a QR code for the iPhone app. It also lists phones and removes access.
`mobile --status` reads the saved configuration without enabling anything.

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
  `desktop_sessions`, `machine_id`, `portfolio`, `repository_path`, `view_plan`,
  `all_view_plans`. Repository IDs resolve on the
  destination with `repository_path`; paths in local reads are destination paths.
  `view_plan` returns the existing WorkViewPlan, including the recorded primary
  Session; roadmap membership alone cannot select one. A missing primary stays
  absent. Resolve its owner with `session_owner` before attachment.
- `all_view_plans {expression}` applies the existing selection across repositories
  in one store snapshot, returning existing ViewPlans. `lf desktop plans --all
  --expression 'kind:task state:open' --json` is the same read. It observes local
  Work without synchronously refreshing an external tracker.
- `subscribe {query,generation}` supports `roadmap` and `all_view_plans`. `reading {generation,frame}`
  wraps the existing complete `answers/unavailable/body` watch frame. Subsequent
  client frames are existing watch checkpoints (`{"id":3}`) or argument changes.
  The shared watch validates them, including its 16 KiB limit; an invalid
  checkpoint ends the watch with an `unavailable` reading.
  Closing cancels. Reconnect opens a fresh subscription; no replay cursor exists.
- `command {change}` supports `session_rename`, `session_start` and `task_create` using the existing `Write` shape
  and argv mapping. `command {trace_id,output,stderr,code}` reports the real CLI result.
  The CLI owns mutation, defaults and Process attribution. Transport IDs correlate
  diagnostics; they do not deduplicate commands. Session rename has no automatic
  resubmission. Task creation has its separate durable admission below.
- `session_start {id}` delegates `lf resume SESSION_ID --start-only --json`.
  This explicit action reuses or starts the recorded Session's retained owner,
  returns its existing SessionRecord and takes no viewer claims. CLI, Desktop and
  phone share startup exclusion. Reconnecting only looks up the owner; a lost
  opening reply requires inspection, never automatic launch replay. A later
  explicit open can restart an ended provider; this is not an idempotent receipt.
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

## Recover Task creation

```sh
lf task create 'Fix the retry test' --run --json --request-id 00000000-0000-4000-8000-000000000001
```

Generate a fresh UUID for each intended Task. Keep that UUID and the exact request,
repository, Wave, Graph and launch options until the result is known. Repeat those
arguments after a lost reply. The shared CLI reserves one Task ID before filing;
the existing planning writer keeps external creation on that same Task. It records
the launch attempt before invoking ordinary `task run`, then saves the outcome.
The Machine service delegates this command; it owns no separate receipt cache.

The JSON outcome contains `task_id`, `state`, optional `receipt`, optional
`message` and optional `owner`. `started` means the initial run returned successfully and its conversation was admitted;
it does not mean the Graph finished or that its terminal provider is running.
`in_progress` means another process holds this request. `failed` preserves the
filed Task after a start/admission error. `unknown` means a process exited across
the launch window: inspect the Task before explicitly requesting another run.
Repeating creation never advances an already attempted Graph. All four outcomes
exit zero; invalid or conflicting requests fail normally. Stored outcomes describe
the original operation, not the Task's current status.

Add `--terminal` to explicitly open that primary Session and acknowledge its
retained `owner`, without attaching a viewer. It requires recoverable admission
and cannot combine with `-b` or `-i`. Desktop delegation and this operation share
the same Session startup exclusion and existing owner. Repeating a saved creation
result after its terminal exits does not launch it again.

The typed `task_create` write carries `directory`, `request_id`, `request`, optional
`wave`, optional `graph` and required `terminal`. It requests ordinary interactive
creation, never headless execution or Auto-Ship. iOS uses `terminal: true` only for
the explicit Create & start action; existing-Task attachment never launches.
