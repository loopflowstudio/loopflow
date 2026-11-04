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
Remote callers use the separate Tailscale admission below.

## Connect a phone through host Tailscale

```sh
lf machine service mobile            # enable access and print the address
lf machine service mobile --status
lf machine service mobile --off
```

Sign into the same Tailscale account on the Machine and phone. In Desktop,
open Settings → Mobile → **Enable mobile access**. In iOS, choose **Scan QR code**,
then **Connect Machine**. **Enter address** accepts the same HTTPS address without
camera access. The QR contains only that address; it does not expire or grant access.

Setup finds the installed host CLI, obtains the existing Machine service and
binds HTTPS to the Machine's Tailscale addresses on one available port. No Serve
or Funnel configuration changes. Its endpoint, approved node/user identity and
enabled setting live in Machine config.yaml. The service saves approval before
serving phones, so losing the setup reply still leaves one recoverable address.
The supervisor restores that same approval; mobile failures leave local Desktop
running.

`mobile --status` reports the saved `configuration`, actual listener `ready` state
and any current `error`. It reads the existing service without starting it. Saved
approval alone does not mean the Machine is reachable. Desktop displays the QR
when ready, preserves the address when unavailable, and can recover an uncertain
setup result by reading status without replaying the command.

For each connection, the host's `tailscale whois --json` identifies the actual TCP
peer. Only another untagged device belonging to the approved owner is admitted.
The server ignores caller-supplied identity headers. Same-host network callers
are refused because a Tailscale node cannot distinguish its local OS accounts;
local Desktop/CLI use their private same-user Unix socket. This also means an iOS
Simulator on the serving Mac needs an explicit identity fixture for integration
checks. A physical phone connects as its own Tailscale node.

HTTPS uses the host's Tailscale certificate, retrieved directly into memory and
refreshed every six hours with at least 24 hours' requested validity. Keys are
never written to Loopflow config or logs. MagicDNS and HTTPS certificates must
be enabled for the tailnet. Tagged hosts/clients are unsupported by this personal
Machine path. Browser origins, query credentials and foreign identities are refused.

The service checks host identity every five seconds (CLI calls have a 15-second
bound). Logout or a failed check closes remote connections. Restoring the same
account/address retries the saved listener; a different account/node requires
explicitly enabling access again. `mobile --off` saves disabled state and closes
remote connections without stopping local viewers or owners. Manage individual
phone access through Tailscale; Loopflow has no phone tokens or device-grant list.

`/v1/machine` carries the shared length-prefixed protocol over native WebSockets,
followed by owner frames after `bridged`. Message boundaries have no application
meaning. Binary chunks are capped at 64 KiB; slow readers disconnect without
stopping execution. Ending a stream sends a bounded WebSocket close. Idle streams
answer pings; the foreground phone checks liveness every 20 seconds with a
five-second response deadline. Failed input is never replayed.

[Run the native proof](../../../../swift/LoopflowiOS/README.md#opt-in-host-tailscale-acceptance).
The proof declares its host-identity fixture; it does not replace physical-device
acceptance of installed Tailscale on both ends.

`wire.rs` owns the Rust types; `swift/Loopflow/Services/MachineProtocol.swift`
mirrors them. Executable fixtures live in `tests/fixtures/dto/machine/`.

A request has required `version`, `id` and `operation`. A response has required
`id`, `instance` and `result`. Unsupported revisions fail before reading Work.
The bootstrap result reports the Machine, service build and executable SHA-256,
revision, limits and capabilities. Each restart changes `instance`. Default-Machine
setup delegates to the promoted installation; source builds use an explicit
`LF_HOME` to keep experiments separate.

- `read {query}` uses the existing `Read` shape. Supported reads: `roadmap`,
  `desktop_sessions`, `session_page`, `machine_id`, `portfolio`, `repository_path`, `view_plan`,
  `all_view_plans`. Repository IDs resolve on the
  destination with `repository_path`; paths in local reads are destination paths.
  `session_page` accepts an optional Task filter before its 100-row limit and
  returns the CLI's existing SessionPage with a continuation cursor. Selecting a
  row never changes the Task's primary Session or takes terminal control.
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
