# Deliver Desktop and iOS against one Machine backend

Jack Heart requested parallel backend/frontend work and invoked launch-plan on
November 3, 2026. A reuses LOO-593; B reuses LOO-718, this checkout and primary
conversation. Both feature Graphs stop for interactive demo; no Auto-Ship.
C–F are named follow-ups, not newly filed or launched Tasks.

| Outcome | Owner / dependency | Acceptance boundary |
|---|---|---|
| A: local Machine service | LOO-593 | Installed local service; representative Task subscription, existing command and Session bridge. CLI/socket state and attribution parity. Quit/restart/update preserve independent execution under the actual supervisor. Read errors, cancellation, bounded clients and version/consent failures remain honest. |
| B: native Task → Session client | LOO-718 | Buildable Swift iOS target, Desktop styling, real terminal input/rendering, explicit fixture mode. Existing domain DTOs and backend codec; native keyboard/paste/selection/IME/accessibility proof. No live connectivity claim from fixtures. |
| C: connect the phone over host Tailscale | A + B | Pair intended Machine, list actual open Tasks, attach/edit the same Session. Real Desktop handoff, background/network recovery, bounded output reset and no replay. Select carrier, admission, takeover and first hosting tier before dependent implementation. |
| D: complete Desktop Work cutover | A; parallel with C | Inventory and remove all migrated Work FFI consumers/exports/wiring. Preserve renderer FFI, app-owned automation, reads/writes/watches and authoritative CLI behavior. No permanent direct-FFI fallback. |
| E: recover create/start | shared CLI admission | Same request/payload recovers original Task or honest unknown outcome after lost reply/crash. Preserve a Task if launch fails; defaults/destination/attribution remain shared. No implicit headless/Auto-Ship approval. |
| F: start work from phone | C + E | Jack's selected describe-first sheet: editable typed/dictated text, visible changeable repository/Machine, explicit Create & start. Preserve draft and destination; recover admission without duplicate Tasks. |

A and B begin together. A owns serialization truth and shared executable fixtures;
B supplies the native consumer. The first live joint demo is existing Task →
editable Session → reconnect to the same provider. Supported release requires
A–D; creation needs E–F too. No universal SDK or frontend domain model.

Shared contract needs Machine/service identity and capability negotiation,
complete Task readings with unavailable/cancel semantics, authoritative primary
Session selection, attachment snapshots/output/gaps, separate input/geometry
claims and uncertain-input handling. Terminal recordings must match the actual
owner protocol. Work revisions are not a replay log; request IDs alone are not
durable command receipts.

November 4 reconciliation: Jack Heart authorized sustained native improvement
beyond the initial fixture. The local backend is merged into this checkout;
WorkViewPlan and the phone’s owner decoder are implemented. C now has a separate
authenticated WSS carrier and native Keychain pairing. Simulator acceptance passed
through actual host Tailscale against an isolated empty registry; editable owner
handoff and a separate physical phone remain unproven. mobile-ios.md records
current evidence and remaining work. The same-user Unix socket is never proxied
for remote admission. Automatic setup and installed remote enablement remain open.
Terminal activity #1809 is included through main sync at ac8e088f5f; other
coordination references below do not assert delivery status.

Current backend design:
`/Users/jack/src/loopflow.loo-593-decide-the-desktop/scratch/mobile-backend.md`.
Current client: [mobile-ios.md](mobile-ios.md). Interaction:
[prototype-tasks.html](prototype-tasks.html), [creation study](explore.html).
Before another lane launches, its accepted design/assets must enter its checkout;
references to another checkout do not automatically supply context.

Coordination remains: #1807/LOO-582 Session identity; #1809/LOO-720 activity;
#1791/LOO-673 Task-list behavior; #1808 shared DTO revert (Task attribution
unresolved); #1656/LOO-312 installation; #1805/LOO-714 defaults.
LOO-717 owns shells, LOO-498 Rex workspaces, LOO-709 retention, LOO-710 snapshots,
LOO-712 Session transport. Current implementation must preserve their owners.
Future hub, push, TTS, previews and broad shells remain outside this slice.

Original full plan, acceptance and coordination evidence:
`9e4587bb53fc4102e8682a3bba61e9776a6de5c2:scratch/mobile-task-plan.md`.
