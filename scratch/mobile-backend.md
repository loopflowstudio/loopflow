# One Machine backend for Desktop and phone

The working backend design moved to [LOO-593’s checkout](/Users/jack/src/loopflow.loo-593-decide-the-desktop/scratch/mobile-backend.md) at implementation handoff. This file preserves the reviewed design as reference; do not evolve a competing backend specification here. [Native iOS implementation](mobile-ios.md) is the current lane for LOO-718.

LOO-718 · Backend reference snapshot · November 3, 2026 · Product

Jack Heart selected native Swift, host Tailscale, terminal Sessions, CLI command delegation and the describe-first new-Task sheet. Jack accepted the general direction of replacing Desktop’s Work C binding with a socket protocol, then asked whether this repeats lfd’s mistakes. Detailed cutover, packaging and protocol choices remain under evaluation, not implementation approval or a selected server name. [Installation and lifecycle research](<reports/Machine service installation and lifecycle.md>) distinguishes verified behavior from recommendations.

[Open the shareable design review](mobile-backend-review.html): architecture scenarios, historical lessons and five unresolved choices. Page selections are review notes only; they do not update this design.

[Task plan](mobile-task-plan.md) keeps outcomes, dependencies and launch coordination separate from these design notes. Backend and native iOS work begin in parallel against a shared application contract.

## What to build

A standalone Machine service provides one typed Work contract to Desktop over a private local socket and to iOS over host Tailscale. Desktop automatically obtains its local backend; phone access separately enables remote admission.

```text
Swift Desktop ── local Unix socket ────────────┐
Swift iPhone ── host Tailscale / HTTPS + WSS ──┼── Machine service
Future optional hub ── authenticated ingress ─┘          │
                                  ┌────────────────────┼─────────────────┐
                                  ▼                    ▼                 ▼
                         shared Rust reads       lf subprocess     owner socket bridge
                         and subscriptions       typed command     terminal / control
                                  │                    │                 │
                                  └── Machine store ───┘           Session owner
                                                       │                 │
                                                  Graph runner      agent and PTY

Desktop retains its UI automation endpoint and native terminal rendering.
```

## Current system and cutover boundary

Today `RegistryQueryLocal` wires Work queries through `LoopflowLibrary` to `lf_read`, `lf_write` and `lf_subscribe`; other commands already invoke `lf`. `DesktopServer` is a separate app-lifetime UI automation endpoint. There is no general Machine listener or installation yet. The proposed service runs as the existing Machine's OS account and data directory, without requiring Desktop windows.

Preserve `RegistryQuery` consumers and domain DTOs where their semantics fit. Replace the Work transport with a socket client. Shared methods have request IDs, typed errors, protocol capabilities and service-instance identity; subscriptions add generation/cancellation and bounded delivery. Local and remote transports share application semantics, not assumed peer identity. 

- **Reads and subscriptions:** call existing Rust `Read::read` and `Read::subscribe`. These already share `lf watch` behavior. Start with complete replacement readings and explicit unavailable state; do not invent replay cursors from store revisions. Run blocking reads away from network handling and cancel unused watches.
- **Commands:** accept typed operations and delegate through installed `lf` argument arrays, reusing command mappings. Resolve repository IDs on the destination. Internal commands execute locally without calling back into the service; avoid service→CLI→service recursion.
- **Terminals:** bridge existing Session-owner sockets, with separate bounded streams and input/geometry claims. No new PTY manager or process per keystroke. Continue ended Sessions explicitly; stale attachment never creates a replacement conversation.
- **Native UI:** window focus, visible pane arrangement and menus remain in Desktop. Keep Ghostty rendering FFI. Device presentation is not Machine execution authority.

**One complete operation:** resolution, defaults, state writes, attribution and recovery have one shared owner. Current FFI already shares the engine; moving it adds a process boundary. CLI and socket adapters are valid, and CLI need not depend on the service. No service-specific Task orchestration or durable business-receipt implementation. [lfd history](<reports/Lessons from earlier lfd attempts.md>) shows drift around a shared executor (#304), an unfinished exec cutover (#825), and shared functions with competing lifecycle owners in the second daemon.

## Automatic installation and lifetime

Recommend a supervised foreground Rust service with one installation identity per Machine. Desktop resolves the authoritative installation, registers/bootstraps a per-user LaunchAgent when needed, connects to its private socket and verifies readiness. Normal local use needs no Tailscale. Show disabled Background Items or incompatible versions honestly; do not re-register around an explicit macOS disablement.

`SMAppService` fits a helper in the selected signed bundle. A GUI-less signed container can provide standalone Mac deployment. Exact packaging remains open: app-bundled helper versus independently promoted container. Existing installation promotion and the public `lf` entry must own both Desktop and CLI setup, rather than competing installers. Development builds use explicit separate homes/identities. 

Helper updates can require unregister/re-register, which terminates the helper. Closing Desktop or replacing the network service must leave Graphs and Session owners running. Prove this with actual launchd cleanup and Linux service units; process detachment alone is insufficient. Personal LaunchAgents stop at logout. Sleep makes the host unavailable; reboot preserves records, not processes. Before-login hosting is a separate deployment promise. Uninstall removes only owned service registration/artifacts, preserving Work and credentials; define active-session handling before implementing removal.

## Public operations and trust

Proposed names, not implemented endpoints:

| Operation | Meaning |
|---|---|
| `bootstrap()` | Authenticated Machine identity, protocol/capabilities, repositories and destinations |
| `subscribe_tasks(repository_id)` | Complete readings with unavailable state and subscription generation |
| `create_task(request_id, repository_id, execution_machine_id, text)` | Recoverable create/start admission |
| `request_status(request_id)` | Original result, progress or explicit unknown outcome |
| `attach_session(session_id, resume_cursor)` | Observe an existing owner, then separately acquire control/geometry |

Recommend HTTPS/WSS through host Tailscale Serve, with remote listening enabled separately from local service setup. Keep local same-user peer admission separate from a remote/proxy endpoint: Serve connected to an ordinary trusted Unix socket otherwise appears as the proxy's local UID. Pairing supplies revocable phone credentials and expected Machine identity; reachability is insufficient. Preserve unrelated Serve routes. Exact pairing, protocol compatibility and Swift Unix transport implementation still need specification.

Rex's discoverable schemas, snapshot/event ordering, bounded consumers and separate terminal data capabilities are useful precedents. Reuse existing DTOs with required or explicitly optional fields and Rust/Swift fixtures. Initial phone connections target paired Machines directly; existing SSH routing can remain inside commands with explicit destinations. A future hub uses separate authenticated ingress, never becoming a required terminal relay or scheduling authority.

## Create & start and reconnection

The sheet names its last valid repository/Machine default and permits changing it. Existing `lf task create REQUEST --run --json` returns Task, Graph and optional Session identity; it starts the workflow and admits the conversation. Match Desktop; confirm launch behavior. Do not silently substitute conversation-only launch or pass headless/ship approval.

Persist request identity and exact payload at the shared command boundary. Correlate the Task and provider receipts so a lost reply cannot create it twice. A gateway response cache alone cannot close a crash after external creation. Unreconcilable delivery remains outcome unknown. If creation succeeds and launch fails, return the existing Task and recover its launch rather than refile it. Honor the displayed destination against placement defaults; never silently rent another Machine.

Work reconnects with fresh readings; terminals resume bounded output or replace their screen. A new service connection must not steal a newer controller claim. Never replay uncertain raw keys or Send. Voice drafts stay on the phone until deliberate submission; speech engines need not enter the initial server API.

## Telemetry worth preserving

Jack Heart accepted preserving useful telemetry while simplifying its mechanisms. Keep Work history and attribution, provider-authored usage with explicit evidence gaps, and enough diagnostics to distinguish client, transport, service, command and provider failures.

Commands retain the shared `lf` Process recording path. Provider usage remains attached to its conversation and is counted once, regardless of viewers or reconnects. Reads and subscription updates do not become Work records. Durable execution and recovery state is not optional telemetry.

Carry request identity, authenticated client origin, destination and resulting Process/Task/Session identities across the service boundary. Correlation is separate from execution ancestry: forwarding a request does not make the service its Graph owner. Keep concurrent requests isolated; never mutate process-global caller identity for individual requests. A dropped connection is not evidence that the command failed.

Start service diagnostics with aggregate latency/errors, active connections/subscriptions, queue pressure and reconnects. Individual failures can retain bounded, correlated diagnostics; every snapshot and terminal byte needs no durable event. Exact fields and retention remain implementation decisions.

Acceptance: start work from the phone, disconnect, restart the service and reconnect. The retained evidence must explain what ran, its actual outcome (or explicit uncertainty), and provider usage without duplicate accounting. Include simultaneous requests so attribution cannot bleed between callers.

## Cutover and proof obligations

Prove representative Desktop Work reads/subscriptions, commands and Session attachment through the installed local service. Native iOS renderer/input work proceeds alongside this proof, consuming the same contract and fixtures. Live phone integration joins the two paths; it need not wait for every Desktop operation to migrate. Complete the Work cutover before treating the architecture as supported. The separate [task plan](mobile-task-plan.md) owns sequencing and delivery boundaries.

**Delete — do not maintain:** migrated `RegistryQueryLocal` FFI closures, `LoopflowLibrary`/`ServedWatch` Work glue, and unused `lf_read`/`lf_write`/`lf_subscribe` exports with exclusive build/test wiring after consumer inventory. Move shared Rust implementation rather than copying it. Preserve justified renderer FFI, CLI operation code, SSH and Desktop UI automation. No permanent direct-FFI fallback, resurrected lfd store or second retention authority.

Prove CLI/socket parity in resulting Task/Session state, defaults and Process attribution; compare initial/change/error/cancel watches. Lose a create reply, restart and retry the same identity: recover one Task or report unknown. Preserve a created Task after Graph failure; remote transport failure must cause no local mutation. Bound slow-client resources and reject stale control. Under actual supervisors, quit Desktop and replace the helper while preserving the same provider/owner; concurrent startups produce one service under one installation/schema authority. Check disabled consent and CLI/service version skew. Real-device review completes Task → editable Session → background/reconnect. Exact commands follow implementation modules; packaging, carrier and handoff remain open.
