# Rex machine service installation and lifecycle

Research date: 2026-11-03. Jack requested evidence for Loopflow's Desktop-managed machine service and a possible shared socket protocol for Desktop and iOS. Public Rex documentation and narrow, read-only inspection of the installed Mac app informed this report. No Rex command was executed that could activate its server; no service was changed.

## Does Rex Desktop install and supervise a separate Mac server?

### Takeaway

The installed Rex Beta demonstrably has a separate server running as a per-user launchd LaunchAgent. Public documentation says the Mac app starts a local server over a Unix socket; neither source establishes its complete registration, login, logout, or uninstall policy. [Local observation](#local-observation); [Privacy](https://www.superlogical.com/rex/docs/reference/privacy)

### Cited Findings

- Mac Rex uses a local Unix-socket server, initially without network listeners or Tailscale. Remote connections are explicitly added; the app reconnects to configured servers rather than discovering arbitrary servers. [Privacy](https://www.superlogical.com/rex/docs/reference/privacy)
- Mac Settings exposes server running/stopped/preparing/failed state, API version, uptime, compatibility, restart/update recommendations, local API socket URL, and bundled-versus-source server selection. The remote serving toggle and address/certificate readiness are separate. [Mac Settings](https://www.superlogical.com/rex/docs/reference/mac-settings)
- The public Mac actions include `client.server.restart`, which stops and restarts the local server. [Mac Actions](https://www.superlogical.com/rex/docs/reference/mac-actions)
- Installed build 1696 bundles a server helper, and launchd runs an installed copy outside the app bundle. [Local observation](#local-observation)

#### Local observation

Sanitized metadata collected read-only on 2026-11-03:

| Inspection | Observation |
|---|---|
| App Info.plist | `/Applications/Rex Beta.app`, bundle ID `com.superlogical.rex`, version `0.1.0`, build `1696` |
| Bundled helper Info.plist | `Contents/Helpers/Rex Server.rex`, bundle ID `com.superlogical.rex.server`, executable `rex`, same version/build, `LSUIElement=true` |
| Installed executable | `~/Library/Application Support/com.superlogical.rex/RexServer/Versions/Current/Rex Server.app/Contents/MacOS/rex` |
| Other installation metadata | Version directory `1696` and working directory `RexServer/CLI` also exist. `Current` was not a symlink in this inspection. |
| Filtered `launchctl list` | Running `com.superlogical.rex.server.<hash>` job and separate app job |
| Filtered `launchctl print gui/501/<server-label>` | `type=LaunchAgent`, `state=running`, `domain=gui/501`; program is the installed `Current` executable; properties include `abandon process group` |
| Launchd-reported job path | `~/Library/Application Support/rex/servers/<hash>/com.superlogical.rex.server.<hash>.plist`; a subsequent path/glob check found no corresponding file |
| Narrow process metadata | Main app and server are separate processes with parent PID 1; another `rex` process from version `1696` has the app as parent, but its role was not established |
| Conventional registration locations | No matching Rex plist found in `~/Library/LaunchAgents`; app `Contents/Library` absent |

Only selected launchctl fields and process command paths were emitted, never environment values or full process arguments. App/helper plist fields were selected, not dumped. This observation establishes a loaded supervised service, not durable registration at login or the cause/timing of installation.

### Inferences

- Loopflow can adopt the same product boundary: Desktop provides installation/status/control, while a separate user service owns the machine API. A service binary should also be independently installable on a remote machine without Swift UI.
- Installing the local service and enabling remote access should remain distinct operations. An immediately usable Desktop should not implicitly authorize tailnet access.
- The external versioned installation suggests an upgrade strategy worth studying, but filesystem layout alone does not prove how activation, rollback, cleanup, or session preservation works.

### Gaps

- No reliable public Mac contract found for first-launch registration, automatic login launch, logout behavior, app-quit behavior, uninstall cleanup, or disabled Background Items.
- No evidence that Rex uses `SMAppService`; launchctl metadata does not establish that implementation.
- The missing source plist means the loaded job cannot be treated as proof of persistent login registration. No lifecycle experiment was run because active Sessions must remain undisturbed.

## What persists, and what happens during upgrades?

### Takeaway

Rex separates viewing from execution. Its Linux documentation makes restart-loss boundaries explicit, but those Linux policies must not be advertised as verified Mac behavior. [CLI concepts](https://www.superlogical.com/rex/docs/automate/cli); [Server modes](https://www.superlogical.com/rex/docs/linux/server-modes)

### Cited Findings

- A Rex Session groups work and remains running without viewers. A Rex window is a tab inside that Session, a block is an application, and a client is a connected program. Rex puts the running server's CLI directory first in terminal PATH and supplies Session/block/server context. [CLI concepts](https://www.superlogical.com/rex/docs/automate/cli)
- On Linux/FreeBSD, personal servers lazily autostart and exit when idle; explicitly started servers persist. Login servers delegate execution to per-account servers that survive the login frontend stopping. [Server modes](https://www.superlogical.com/rex/docs/linux/server-modes)
- Linux shared-server restart ends Sessions. Existing personal/per-account servers may continue on the old version until their work drains. Installed and running versions differ; clients tolerate some version skew and reject incompatibility. [Upgrading](https://www.superlogical.com/rex/docs/linux/upgrading)
- `rex server stop --if-idle` refuses while Sessions or other clients remain, and refuses if an older server lacks that feature. Explicit server start is persistent and idempotent. [CLI reference](https://www.superlogical.com/rex/docs/reference/cli)
- Current Mac Settings describes experimental background updating; Privacy describes chosen downloads. Current `client.update.accept_when_idle` documentation refers to windows being out of sight, not programs completing. These pages do not establish a safe execution-drain contract for Mac updates. [Mac Settings](https://www.superlogical.com/rex/docs/reference/mac-settings); [Privacy](https://www.superlogical.com/rex/docs/reference/privacy); [Mac Actions](https://www.superlogical.com/rex/docs/reference/mac-actions)

### Inferences

- Loopflow should specify separate guarantees for UI quit, API-service restart, owner-process restart, logout, and machine reboot. “Retained” must state which boundary it survives.
- Detached Session-owner processes could let the machine API restart without killing agent PTYs. That is a Loopflow design proposal, not a claim that Rex Mac uses this decomposition.
- Show both installed and running versions; retain the executable needed by active owners; drain old owners intentionally. A restart/update control must explain whether it kills execution.
- A successful mobile connection must not require Desktop to be visibly open. That argues for an explicitly managed user service rather than Desktop merely parenting a child process.

### Gaps

- Mac update choreography, old-version cleanup, crash recovery, orphan reaping, resource limits, and actual app-quit retention remain unverified.
- A per-user GUI LaunchAgent does not itself promise remote availability after logout or before login. Loopflow must choose its own scope.

## Can Desktop and mobile share a protocol without merging GUI state into execution?

### Takeaway

Rex provides strong precedent for one server-owned domain API with different local/remote transports, explicit capability discovery, and separate terminal streams. Its client actions remain distinct from server-managed Session layout. [Server API guide](https://www.superlogical.com/rex/docs/automate/server-api); [API reference](https://www.superlogical.com/rex/docs/reference/api)

### Cited Findings

- `rex api list/describe/call` discover and invoke the connected server's API. Machine-readable JSON Schemas, examples, and validation notes match that running version; CLI and Lua derive their interfaces dynamically. Unknown methods and invalid arguments produce distinct errors. [Server API guide](https://www.superlogical.com/rex/docs/automate/server-api)
- Local identity can be a Unix peer; remote identity can be Tailscale. Some administration methods accept local sockets only. API revision/minimum revision are reported. [API reference](https://www.superlogical.com/rex/docs/reference/api)
- Attach/bootstrap combines snapshots and sequenced events; clients apply events after the snapshot sequence. Slow consumers exceeding 8 MiB disconnect and resnapshot. [API reference](https://www.superlogical.com/rex/docs/reference/api)
- `data.connect` issues expiring, single-use, user-bound tickets for `/api/data` WebSockets, optionally bound to a subprotocol such as `rex.data.terminal.v1`. [API reference](https://www.superlogical.com/rex/docs/reference/api)
- Client-native actions such as `pane.split` cannot run as server actions. `client.perform` asks a named client to perform its declared action. Server Session windows/layout are separate. [API reference](https://www.superlogical.com/rex/docs/reference/api)
- Detach preserves execution; close ends programs. Block ownership resolves competing terminal size preferences and grants no additional rights. [API reference](https://www.superlogical.com/rex/docs/reference/api)

### Inferences

- For Loopflow, replace domain-level Desktop C-binding calls with a typed client interface implemented over the local Unix socket; use the same requests/results/events over a tailnet connection on iOS. Keep Rust domain/storage implementation behind the service. This is a proposed boundary, not a wholesale requirement to remove every FFI dependency: terminal rendering may retain native bindings.
- Keep shared reads/subscriptions native in the service. Command requests can dispatch to `lf` subprocesses without the client depending on CLI text output or process launch details. Avoid a service→CLI→service recursion by making internal local command execution explicit.
- A transport-independent envelope should include request IDs, typed errors, protocol/version capabilities, and a server-instance identity. Subscriptions need an atomic snapshot/sequence contract, bounded buffering, reconnect behavior, and cancellation. Those are more important than copying Rex method names.
- Keep terminal bytes/frames off the Task-list subscription path. Use a narrowly authorized attachment to an existing Session owner; define resize/input authority separately from passive observation. Determine full terminal recovery semantics before choosing raw PTY bytes versus state snapshots.
- Keep window focus, menu presentation, visible pane arrangement, and navigation on the client unless a cross-client layout is intentionally a product feature. Domain operations should target Task/Session IDs, not whichever UI pane happens to be active.
- A future hub can become another authenticated client of this domain API, receiving webhook-triggered work through an explicit outbound link. It should not own local PTYs or duplicate storage ownership. No hub implementation is required now.

### Gaps

- Public material inspected does not specify the entire control wire framing, every endpoint, binary terminal stream encoding, or client resume handshake. Do not claim that copying the documented methods is sufficient to implement a compatible Rex client.
- No direct packet inspection or source access was used. Shared local/remote API semantics are well supported; identical implementation details across every transport remain an inference.

## What should host Tailscale change?

### Takeaway

Host Tailscale can preserve the same server API while changing connection admission. The local peer socket and a socket accepting a network proxy are different trust boundaries. [Remote access](https://www.superlogical.com/rex/docs/linux/remote-access)

### Cited Findings

- Rex documents both host Tailscale Serve forwarding to a dedicated socket and direct tailnet-address listeners. It warns that using a normal Unix listener behind Serve incorrectly identifies callers as the proxy's local account. The dedicated `tailnet-serve://` listener handles that boundary. [Remote access](https://www.superlogical.com/rex/docs/linux/remote-access)
- Rex currently relies on transport-provided identity, with more complete authentication/authorization planned. Host ownership is the default admission policy for personal/shared access; tagged devices require special handling. Its host list currently does not sync between CLI and GUI. [Remote access](https://www.superlogical.com/rex/docs/linux/remote-access)

### Inferences

- Loopflow should offer one local Unix endpoint for trusted same-user Desktop access and a separately authenticated tailnet endpoint/proxy socket. Sharing the method implementation must not accidentally equate the proxy's Unix UID with the remote caller.
- Detect host Tailscale status, account, addresses, and reachability; expose actionable state. Detection is not admission: remote enablement and permitted identities still need a clear product contract.
- The Mac install experience should distinguish “service ready,” “host Tailscale signed in,” “remote endpoint reachable,” and “this device authorized.” One generic connected indicator hides the failures users need to fix.

### Gaps

- macOS host Tailscale packaging/Serve support and Loopflow's exact identity verification path are outside this Rex investigation. They must be verified before selecting Serve versus direct tailnet listening.
