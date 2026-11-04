# Application protocol and transport

2026-11-03 · Research for Jack Heart · Proposed private Machine API

**Define the Loopflow application contract independently of the carrier. Recommend HTTPS/WSS over host Tailscale for the native phone; retain SSH as a real alternative to test, not as an inferior terminal-only design.** Both require the same answers about identity, Session lifetime, mutations and recovery. Neither requires a custom relay or public hub.

## Start from the implemented application boundary

Current [Rust Read](../rust/loopflow/src/lf/commands/read.rs) and [Write](../rust/loopflow/src/lf/commands/write.rs) enums describe operations already shared with Desktop through [Swift RegistryQuery](../swift/Loopflow/Services/RegistryQuery.swift). Reads return the same typed output as corresponding CLI commands; writes call the same operation and record the same Work process where applicable. Remote routing can still delegate to the CLI. This is a stronger starting point than exposing the Desktop automation socket or rebuilding an independent REST domain model.

Keep three surfaces: typed queries/mutations, Work observations, and binary terminal attachment. A single server executable may implement all three, but their lifetimes and recovery semantics remain separate. Reuse Session, Task and Machine identities; do not import Rex’s block/window hierarchy as another definition of Loopflow work.

A generic argv endpoint is cheap to prototype and comprehensive, but makes CLI parsing, ambient directory resolution, output conventions and side effects part of the mobile protocol. It obscures operation-specific authorization and retry semantics. Existing `Write::argv` equivalence is useful internally; it does not imply the phone should assemble arbitrary shell strings. Keep CLI escape hatches for explicit advanced use, and implement initial native controls through typed operations. The earlier lfd argv adapter in [the history](lfd-history.md) makes this a recurring tradeoff, not a novel simplification.

## What to borrow from Rex

Rex publishes discoverable methods with input/output JSON Schema, examples and validation notes; misspelled fields fail rather than silently disappearing. The method inventory is available through its CLI and machine-readable documentation. Borrow one operation definition feeding docs, validation and client fixtures. Dynamic discovery supplements a compiled Swift client; it should not turn the core phone experience into automatically generated forms. [Rex Server API](https://www.superlogical.com/rex/docs/automate/server-api)

Its data API issues single-use, expiring tickets for separate WebSocket data connections, optionally bound to a subprotocol. This separates control setup from bulk content. That is a useful pattern if Loopflow adopts distinct terminal sockets; do not copy ticket machinery before deciding whether a single authenticated connection already supplies the needed binding. [Rex data.connect](https://www.superlogical.com/rex/docs/reference/api#data)

Public documentation supports these behaviors, but is not a complete reviewed implementation. Do not promise byte-for-byte Rex interoperability, internal concurrency behavior or undisclosed recovery guarantees. Build Loopflow’s contract from the operations the product needs.

## HTTPS/WSS and SSH compared honestly

SSH can execute a noninteractive command and forward TCP or Unix sockets. A framed typed protocol can run over a subsystem/stdio channel or a tunnel; it need not scrape terminal output or parse CLI prose. [OpenSSH](https://man.openbsd.org/ssh)

WezTerm demonstrates this composition: SSH bootstraps/connects to a remote multiplexer over a Unix socket. A compatible server is still required. The useful comparison is therefore “how does the client authenticate and carry the application protocol?”, not “API versus SSH.” [WezTerm multiplexing](https://wezterm.org/multiplexing.html)

| Question | HTTPS/WSS over host Tailscale | SSH over host Tailscale |
|---|---|---|
| iOS implementation | Foundation provides HTTP and WebSocket APIs | Bundle and maintain an SSH client implementation |
| Host installation | Private listener or Serve frontend, TLS configuration, app identity | Working SSH server/account plus Loopflow endpoint or executable |
| Trust setup | Certificate validation and explicit app/Tailscale identity mapping | Host-key trust and SSH account authentication |
| Stream separation | Several sockets or application multiplexing | SSH channels or application multiplexing |
| Reconnect | Reopen carrier, then restore app state | Reopen carrier, then restore app state |
| Troubleshooting | Standard HTTP status, health and request traces | Familiar SSH diagnostics; channel and remote environment failures |
| Existing Loopflow fit | New network adapter around current core | Existing Machine command routing, but no existing iOS SSH transport |

Apple’s `URLSessionWebSocketTask` supports asynchronous binary/text messages and subprotocol negotiation over TCP/TLS. That lowers native integration cost, but does not make a connection survive phone suspension or provide application replay. [Apple WebSockets](https://developer.apple.com/documentation/foundation/urlsessionwebsockettask)

**Recommendation:** prototype HTTPS/WSS first using the host Tailscale installation and keep framing independent enough for a later SSH carrier. Do not implement both production transports initially. Compare cold connection, foreground recovery, authentication UX and operational burden with a small SSH spike if HTTP setup proves more cumbersome than expected.

Rex supports both host Tailscale Serve and direct tailnet-address listeners, in addition to embedded Tailscale. Its Serve mode distinguishes proxy identity from local Unix peer identity; ordinary Unix-socket identity would identify tailscaled rather than the remote caller. Loopflow must preserve that boundary if using Serve. [Rex remote access](https://www.superlogical.com/rex/docs/linux/remote-access)

## Propose a deliberately small wire contract

Initial handshake returns Machine identity, service incarnation, protocol selection, build information, supported operations, terminal capabilities and limits. Authenticate before returning private Work. Client labels identify surfaces, never grant authority. Scope every resource to its Machine; a Task/Session identifier alone is insufficient in a multi-host cache.

Requests name a typed operation and correlation ID; mutation operation IDs have the stronger durable semantics in [recovery research](research-recovery.md). Replies distinguish validation, unauthorized, unsupported, conflict, unavailable and unknown execution outcome. Transport timeout cannot be translated to “operation failed.” Keep terminal content binary and size-bounded. Preserve terminal-control authorization separately from observation and geometry ownership.

Use a small control stream and independently budgeted terminal streams. This can be separate WebSockets or one carefully scheduled multiplexer; choose from measured contention, not aesthetic uniformity. Large history/file transfers must not block cancellation or control transfer. Advertise limits as protocol facts rather than assuming a transport send callback means the terminal emulator consumed the data.

## Versions: three boundaries, not one app number

1. **Phone ↔ Machine adapter:** independently released products need a negotiated public protocol and capabilities.
2. **Adapter ↔ live owner:** an older owner can remain busy while the adapter updates. Its attachment format and lifecycle policy are a separate compatibility boundary.
3. **Executables ↔ store:** schema reader floors may prevent an older owner reading or writing even when its socket still responds.

Current [owner wire](../rust/loopflow/src/transport/wire.rs) has fixed frame tags and JSON request kinds, plus an `lf1` in-band event marker; it does not expose a general supported-version-range handshake. Current Desktop automation has its own explicit version, which says nothing about owner compatibility. Saved terminal snapshots are tied to the Ghostty revision. Treat these as concrete gaps before promising seamless mixed-version updates.

Rex documents client/server version skew, feature-specific update messages and rejection when versions are too far apart. Its upgrade behavior also differs by server mode: some executions remain on their original version while others end. Borrow explicit compatibility and retirement policy instead of implying all restarts are harmless. [Rex upgrades](https://www.superlogical.com/rex/docs/linux/upgrading)

**Proposed policy:** negotiate a public wire revision with minimum/maximum support, then capabilities for optional behavior. Fail incompatible handshakes with the component requiring update. Maintain only a named, tested compatibility window; normalize older messages once at the adapter boundary. Preserve an old busy owner when compatible, drain it deliberately, and block an incompatible schema promotion or expose the required interruption before applying it.

Paseo explicitly treats app and daemon as independently shipping products, negotiates capabilities and keeps compatibility transformations at the client boundary. Its optional/default-heavy additive-field strategy should not be copied directly: Loopflow’s DTO rules require fields to be required or explicitly optional, without silent defaults. [Paseo compatibility](https://github.com/getpaseo/paseo/blob/main/docs/protocol-compatibility.md)

AHP’s Swift client separates opening a transport from protocol reconnect and publishes supported protocol versions. That layering is useful to study even if its agent-oriented schema is not adopted. [AHP Swift client](https://github.com/microsoft/agent-host-protocol/blob/main/clients/swift/AgentHostProtocol/README.md)

## What must be proved first

Ship no broad compatibility abstraction before validating one old-owner/new-adapter/new-phone combination and its inverse. Round-trip Rust/Swift fixtures must reject missing required fields and unsupported cases. Demonstrate one native typed read, one safe mutation, an observation replacement and terminal reattachment over the selected carrier. Measure whether bulk output delays control. Keep a future public hub on a separate authenticated delivery API; it should not extend the phone’s authority or become a mandatory routing hop.
