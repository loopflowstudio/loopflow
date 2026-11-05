# Loopflow and Rex: server architecture and wire protocol

LOO-593 · interactive demo · November 4, 2026 (Pacific)

Jack Heart requested this comparison. Loopflow source: `0c3759a910`. Rex evidence:
the already-running Mac server, `0.0.1793377904-ge6592f`, API revision 8, its
served `/llms.txt` specification and `api list --json`, plus current public docs.
Read-only inspection; no Rex sessions, settings or installations changed.
Built from `server/{wire,service,client}.rs`, `transport/{wire.rs,README.md}`,
`ServerClient.swift`, `docs/dev/server.md` and `scratch/mobile-backend.md`.

**Keep the independent execution owners. Rex provides a useful model for the
client protocol around them: discoverable methods, structured results and explicit
version negotiation.** Those choices are independent of whether the server owns
running programs. This comparison proposes no implementation change or shipping
approval.

## The system boundary

```text
Loopflow today
Desktop ── framed Unix connections ── lf server
                                      ├─ shared Rust reads / writes / watches
                                      └─ bridge ── existing execution owner
                                                     └─ provider / PTY
CLI ────── shared Rust operations + direct owner access
Desktop automation ── app-owned windows / panes

Rex, as documented by the installed server
App / CLI / Lua ── control WebSocket ── Rex server
                                       ├─ sessions / layouts / blocks
App ───────────── data WebSocket ──────└─ terminal / program
Native client actions ← request routed to the named app
```

The nouns differ. A Rex session groups windows and blocks. A Loopflow Session
is a durable agent conversation; Work and native arrangement live elsewhere.
Equating the two would import layout and process-lifetime responsibilities into
the wrong Loopflow object. Rex can route native actions to a specific client;
that is compatible with keeping our Desktop automation app-owned. Sources:
[Rex method reference](https://www.superlogical.com/rex/docs/reference/api),
[Loopflow boundary](../docs/dev/server.md).

Restart behavior makes the lifetime distinction concrete. Rex's Linux personal
and account servers retain their old executable while sessions remain; restarting
a shared server ends its programs. Its login server is an ingress exception:
restarting it preserves independent account servers. Our access server can be
replaced while existing execution owners continue. The same-build composed
replacement test passed; final rendered continuity, released-version upgrade and
Linux replacement remain unproved. Sources:
[Rex upgrades](https://www.superlogical.com/rex/docs/linux/upgrading),
[Loopflow proof limits](mobile-backend.md#remaining-proof-and-delivery).

## The wire, side by side

**Layer clarification — November 5:** Jack Heart asked about within-computer
requests. A Unix socket is a local transport; WebSocket supplies a handshake and
message protocol above a stream. WebSocket can run over a Unix socket, as Rex's
local endpoint does. Persistence is a third choice: our one-operation connections
are an implementation decision, not a Unix-socket limitation. The table compares
current stacks, not mutually exclusive technologies.

Rex details below come from the installed server's own protocol specification,
retrieved over its existing Unix socket. They describe that build, not a claim
that every released Rex version behaves identically.

| Boundary | Loopflow in this branch | Rex API revision 8 |
| --- | --- | --- |
| Control carrier | Same-user Unix socket; four-byte big-endian length followed by JSON. One operation per connection, with subscriptions remaining open. | Persistent `/api/control` WebSocket, `rex.control.v1`; JSON text messages carry requests, responses and events. |
| Request and result | `version`, `id`, tagged `operation`; response includes server `instance`. Reads return JSON/text inside an `output` string; commands return `trace_id`, `output`, `stderr`, `code`. | Method name plus typed payload; structured result or error with machine-readable code and HTTP-style status. Requests may overlap; dependent calls await their predecessor. |
| Discovery | Bootstrap advertises broad capabilities and limits. Rust/Swift types and shared fixtures define the wire. CLI help separately describes command syntax. | The connected server publishes input/output JSON Schemas, examples and validation notes; CLI/Lua discovery follows that server. |
| Compatibility | Exact protocol revision 1. Local readiness also checks the selected helper's executable hash. That installation check is separate from bridge admission. | Hello/welcome negotiate an overlapping API revision range. This build advertises revisions 0–8 and documents translation to older shapes. |
| Work updates | Complete watch readings, generation and checkpoint acknowledgements; reconnect opens a fresh reading, without a replay cursor. | Snapshot plus ordered connection events, with a response sequence boundary and domain revisions to avoid applying a change twice. Reconnect reads state again. |
| Terminal channel | `attach(owner)` acknowledges, then switches that connection to the existing owner protocol: one-byte tag, four-byte big-endian length, payload. Input and output share it. | An attached control client obtains an expiring, single-use, principal-bound ticket for `/api/data`. Output uses binary WebSocket messages; input uses control methods. |
| Recovery | Owner reconnect token and consumed byte position; refresh if retained bytes expired. Native recovery preserves newer input/geometry claims and never replays uncertain input. | Control-client and data-stream reconnection are separate. Data resumes by token and consumed byte count; reconnect tokens rotate. Control events and pending replies are not replayed. |
| Bounds | 32 MiB server frames, 64 concurrent connections, five-second writes. Owner stream: 64 KiB chunks, 4 MiB payload/queue bounds, 1 MiB replay spool. | Control messages limited to 1 MiB in this build; 8 MiB event backlog causes disconnection. Data messages carry up to 64 KiB. Both control and data document 60-second reconnect retention. |

Loopflow sources: [server wire](../rust/loopflow/src/server/wire.rs),
[server implementation](../rust/loopflow/src/server/service.rs),
[readiness](../rust/loopflow/src/server/client.rs),
[terminal contract](../rust/loopflow/src/transport/README.md).
Rex's public [API discovery guide](https://www.superlogical.com/rex/docs/automate/server-api)
explains how to inspect the connected build. Reproduce the bounded read with
`rex --autostart=false api list --json`; its `http` object names the served
documentation and control/data endpoints. The inspection found 73 methods.

Complete Work readings let Loopflow clients replace a projection without writing
event reducers. Rex's sequence contract supports finer updates, with more client
ordering rules. The comparison supplies no workload evidence that warrants
replacing our watch model.

## Similar machinery, different contracts

Both terminal streams combine an initial screen with subsequent output and
nonce-tagged APC events. Both use byte positions for stream recovery. Rex's
documented initial data uses Ghostty's snapshot format; Loopflow's live attachment
uses Ghostty's VT formatter, while ended-screen files use Ghostty snapshots.
Shared Ghostty foundations therefore do not make the wire formats interchangeable.

Neither design promises exactly-once mutations after a lost response. An operation
may have completed; inspect state before retrying. Request correlation IDs do not
solve duplicate creation. This remains a shared-command admission question for
mobile, rather than a reason to add a gateway response cache.

Rex block ownership arbitrates presentation such as terminal size and grants no
additional rights. Loopflow separates size/color ownership from exclusive input
control. Our reconnect policy must retain that distinction; importing the name
“owner” alone would obscure it. Sources: installed Rex `session.own_block` schema
and validation notes; [Loopflow viewer stream](../rust/loopflow/src/transport/README.md#viewer-stream).

## What the comparison suggests

**1. Keep execution independent of client access.** That boundary protects live
providers and Graphs during access-server updates. Rex's login/account split also
demonstrates that an ingress process need not own the programs it reaches.

**2. Make the mobile API discoverable and return domain values.** A rename should
return its identity/result directly, with diagnostics alongside it. Today our
inputs are typed, but many results retain the CLI stdout/stderr shape and require
a second decode. Derive schemas and results from the existing shared operations;
avoid another method catalogue or mutation implementation. Preserve ordinary CLI
formatting at its adapter. This is a proposed improvement, not accepted scope.

**3. Decide compatibility before treating the phone protocol as stable.** Desktop
and its selected local helper can update together. A phone and its host may not.
Rex's revision negotiation makes that difference explicit. A small supported
version window may suffice for Loopflow; copying eight compatibility layers is
not implied. Keep executable selection an installation concern.

**4. Choose framing and connection lifetime without moving ownership.** A
persistent connection can carry requests and subscriptions with either framing
choice. WebSocket over Unix locally and WSS remotely could share message handling
while retaining local peer credentials and separate remote authentication. This
adds HTTP/WebSocket machinery locally; library support for Unix streams must be
checked in each client. Keeping custom local framing is smaller at that boundary,
but leaves two framing implementations. Neither the comparison nor the saved
latency measurements establishes a performance winner. Protocol reuse is a
separate reason to consider the change. Sources:
[WebSocket specification](https://www.rfc-editor.org/rfc/rfc6455),
[WebSocket over Unix example API](https://websockets.readthedocs.io/en/stable/reference/asyncio/client.html#websockets.asyncio.client.unix_connect),
[Unix socket semantics](https://man7.org/linux/man-pages/man7/unix.7.html).

Rex currently derives remote admission from the connection's identity, commonly
Tailscale, and documents broader authentication as planned. It specifically warns
that forwarding the ordinary trusted Unix listener misidentifies remote callers.
LOO-593 supplies local access; LOO-718 owns remote admission and iOS. Its pairing
design and combined integration need their own review. Sources:
[Rex remote access](https://www.superlogical.com/rex/docs/linux/remote-access),
[LOO-718 handoff](mobile-backend.md#loo-718-integration-handoff).

## Questions for the next design pass

These are comparison choices, not new acceptance gates for the published PR.

1. Protocol scope: **a)** preserve custom local framing and improve the shared
   operation contract first; **b)** use WebSocket framing over Unix locally and
   the network remotely. Assess client-library fit and removed code before choosing;
   both preserve the local Unix transport. Jack has not selected either approach.
2. Result contract: **a)** expose typed domain results and generated discovery;
   **b)** retain CLI-shaped replies and decode them in each client.
   **a** is the larger API simplification suggested by Rex.

**Resolved November 5** in [server protocol revision 2](server-protocol-2.md):
question 1 takes the Unix transport with newline-delimited messages locally and
one message per WebSocket frame remotely, on one persistent connection; question
2 takes typed results with out-of-band discovery. That design also records the
survey of other systems Jack asked for.

LOO-593 remains at interactive demo with PR #1813 published. Comparison artifacts
only; no implementation relaunch, merge or Auto-Ship.
