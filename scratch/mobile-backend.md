# One Machine backend for Desktop and phone

LOO-593 · Backend implementation design · November 3, 2026 · Product

Jack Heart selected native Swift, host Tailscale, terminal Sessions, CLI command delegation and the describe-first new-Task sheet. Jack accepted the general direction of replacing Desktop’s Work C binding with a socket protocol, then asked whether this repeats lfd’s mistakes. Jack authorized the local backend implementation lane through launch-plan. The local protocol and selected-app packaging are implemented below; remote admission and standalone Mac distribution remain open. [Installation and lifecycle research](<reports/Machine service installation and lifecycle.md>) distinguishes verified behavior from recommendations.

The original HTML review remains in the LOO-718 checkout. Its recommendations are proposals, not recorded answers. This file is the working backend design; do not maintain a second active backend design in LOO-718.

LOO-718 builds the native iOS client in parallel. This Task owns the local service, its application contract and representative Desktop integration. Full Work cutover, remote pairing and recoverable create/start remain named follow-ups in LOO-718’s task plan. Settle the minimal contract early; publish concrete DTO/fixture and attachment references for the client to consume. Do not make the frontend wait for installed-service packaging to begin native renderer/input work.

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

Desktop now routes roadmap readings/subscriptions and Session rename through the Machine service. Other Work queries still use `LoopflowLibrary` and its shared Rust C bindings until their named cutover. `DesktopServer` remains a separate app-lifetime UI automation endpoint. The Machine service runs as the existing OS account and data directory without requiring Desktop windows.

Preserve `RegistryQuery` consumers and domain DTOs where their semantics fit. Replace the Work transport with a socket client. Shared methods have request IDs, typed errors, protocol capabilities and service-instance identity; subscriptions add generation/cancellation and bounded delivery. Local and remote transports share application semantics, not assumed peer identity.

- **Reads and subscriptions:** call existing Rust `Read::read` and `Read::subscribe`. These already share `lf watch` behavior. Start with complete replacement readings and explicit unavailable state; do not invent replay cursors from store revisions. Run blocking reads away from network handling and cancel unused watches.
- **Commands:** accept typed operations and delegate through installed `lf` argument arrays, reusing command mappings. Resolve repository IDs on the destination. Internal commands execute locally without calling back into the service; avoid service→CLI→service recursion.
- **Terminals:** bridge existing Session-owner sockets, with separate bounded streams and input/geometry claims. No new PTY manager or process per keystroke. Continue ended Sessions explicitly; stale attachment never creates a replacement conversation.
- **Native UI:** window focus, visible pane arrangement and menus remain in Desktop. Keep Ghostty rendering FFI. Device presentation is not Machine execution authority.

**One complete operation:** resolution, defaults, state writes, attribution and recovery have one shared owner. Current FFI already shares the engine; moving it adds a process boundary. CLI and socket adapters are valid, and CLI need not depend on the service. No service-specific Task orchestration or durable business-receipt implementation. [lfd history](<reports/Lessons from earlier lfd attempts.md>) shows drift around a shared executor (#304), an unfinished exec cutover (#825), and shared functions with competing lifecycle owners in the second daemon.

## Automatic installation and lifetime

Recommend a supervised foreground Rust service with one installation identity per Machine. Desktop resolves the authoritative installation, registers/bootstraps a per-user LaunchAgent when needed, connects to its private socket and verifies readiness. Normal local use needs no Tailscale. Show disabled Background Items or incompatible versions honestly; do not re-register around an explicit macOS disablement.

`SMAppService` registers the existing lf helper in the selected signed app bundle. The production registrar passed an ad-hoc-signed container proof. A separately promoted GUI-less signed container remains a future standalone Mac distribution option. Existing installation promotion and the public `lf` entry must own both Desktop and CLI setup, rather than competing installers. Development builds use explicit separate homes/identities.

Helper updates can require unregister/re-register, which terminates the helper. Closing Desktop or replacing the network service must leave Graphs and Session owners running. Prove this with actual launchd cleanup and Linux service units; process detachment alone is insufficient. Personal LaunchAgents stop at logout. Sleep makes the host unavailable; reboot preserves records, not processes. Before-login hosting is a separate deployment promise. Uninstall removes only owned service registration/artifacts, preserving Work and credentials; define active-session handling before implementing removal.

## Public operations and trust

The implemented local revision and executable fixtures are documented in `rust/loopflow/src/machine_service/README.md`. Future remote admission must add authenticated bootstrap, recoverable create/start and request-status lookup at their shared operation owners. Local request IDs currently provide correlation, without durable deduplication.

Recommend HTTPS/WSS through host Tailscale Serve, with remote listening enabled separately from local service setup. Keep local same-user peer admission separate from a remote/proxy endpoint: Serve connected to an ordinary trusted Unix socket otherwise appears as the proxy's local UID. Pairing supplies revocable phone credentials and expected Machine identity; reachability is insufficient. Preserve unrelated Serve routes. Exact pairing and remote protocol compatibility still need specification; the local Swift Unix client is implemented in this slice.

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

Prove representative Desktop Work reads/subscriptions, commands and Session attachment through the installed local service. Native iOS renderer/input work proceeds alongside this proof, consuming the same contract and fixtures. Live phone integration joins the two paths; it need not wait for every Desktop operation to migrate. Complete the Work cutover before treating the architecture as supported. LOO-718’s separate task plan owns the later integration, full cutover and new-Task delivery boundaries.

**Delete — do not maintain:** this slice removed the roadmap read/watch and Session-rename direct-FFI closures from `RegistryQueryLocal.shared`. The full-cutover follow-up removes remaining `LoopflowLibrary`/`ServedWatch` Work glue and unused `lf_read`/`lf_write`/`lf_subscribe` exports with exclusive build/test wiring after consumer inventory. Move shared Rust implementation rather than copying it. Preserve justified renderer FFI, CLI operation code, SSH and Desktop UI automation. No permanent direct-FFI fallback, resurrected lfd store or second retention authority.

Prove CLI/socket parity in resulting Task/Session state, defaults and Process attribution; compare initial/change/error/cancel watches. Lose a create reply, restart and retry the same identity: recover one Task or report unknown. Preserve a created Task after Graph failure; remote transport failure must cause no local mutation. Bound slow-client resources and reject stale control. Under actual supervisors, quit Desktop and replace the helper while preserving the same provider/owner; concurrent startups produce one service under one installation/schema authority. Check disabled consent and CLI/service version skew. Real-device review completes Task → editable Session → background/reconnect. Exact commands follow implementation modules; packaging, carrier and handoff remain open.

## Current implementation boundary and parallel ownership

Deliver plan A: an installed local service, representative Task-list subscription, existing command and existing Session bridge with lifetime/telemetry proof. Do not expand into remote admission, a hub, all Work migration or new-Task recovery in this PR. Resolve technical choices in this design through code and tests; keep product choices such as remote carrier, takeover gesture and unattended Mac hosting explicit. Prototype packaging against the recommended signed helper path, integrate with the existing installation authority, and document any evidence requiring a different package before promising its lifecycle.

LOO-718 owns iOS targets, renderer integration, native keyboard/input, selected Task UI and client tests. LOO-593 owns Rust server/wire contract, shared DTO/fixture truth and the representative macOS adapter. Coordinate Swift package/project shared-file edits through a small consumer patch or commit; do not independently define the same wire shape. Stable schema fixtures and concrete terminal recordings should reach LOO-718 early, before launchd work is complete. Existing target/domain DTOs may be used by the frontend immediately.

Current overlapping PRs: #1809 / LOO-720 terminal activity; #1807 / LOO-582 Session identity and transport; #1791 / LOO-673 Desktop list/FFI semantics; #1656 / LOO-312 app activation/promotion; #1805 / LOO-714 Graph/agent defaults; #1808 Wave DTO revert (Task attribution unresolved). Preserve their behavior and recheck landed status. Original app-only server boundaries are retained in research_notes/desktop-server-original-scope.md. LOO-717 owns shell policy, and LOO-709/710/712 own retention/snapshots/Session transport; this service bridges those owners.

The selected feature Graph starts at implement and stops at demo. This handoff does not approve Auto-Ship. Record installed-supervisor proof separately from unit fixtures.

## Implementation decisions · November 3, 2026

The source research is preserved at `0bd889e4b860fd3d7c44389a86914317c0d22af6:scratch/`; the two synthesis
reports remain here. Historical timeout values in those reports do not override
the current owner policy (currently two minutes of detached quiet by default).

The local slice uses revision-1 length-prefixed JSON on one private Unix socket,
one operation per connection. Task readings use Read::Roadmap/Subscription; the
existing command is Session rename using Write::argv and a real lf subprocess.
Terminal attachment bridges the exact existing owner Process ID. No resume or
provider launch is inferred from a missing owner. The protocol and fixtures live
in rust/loopflow/src/machine_service/ and tests/fixtures/dto/machine/.

Desktop's roadmap and rename closures are replaced without fallback. Other FFI
consumers remain for the named full-cutover follow-up; the canonical Rust Read
and Write enums stay shared. The terminal relay selects the bridge only for
Desktop Session panes, preserving ordinary CLI direct attachment and SSH.

Mac packaging uses the selected signed app's existing lf as the bundled
SMAppService program; source experiments use a private LF_HOME and separately
named launchd registration. No separate binary or store authority is introduced.
Linux uses a user service with normal control-group cleanup because this slice
starts no Graph/provider: it only renames Sessions and bridges existing owners.
Future execution-start operations must prove isolation before entering that unit.

Remaining review: gate owns affected suites, the Xcode link path and Linux user-service replacement. Demo owns actual Desktop quit/reopen, Background Items consent/disablement and phone interaction; the headless owner fixture does not prove rendered input or real-provider usage reporting. The native registration proof uses a temporary signed container, while the live owner/Graph proof uses an isolated launchd job.

Focused checks: `cargo test --lib machine_service::`, the two `transport_tests machine_service_ -- --include-ignored` proofs, `scripts/test_desktop.sh --filter MachineProtocolTests`, the release-plist test, signed-container registrar proof, `cargo fmt` and `cargo clippy --all-targets -- -D warnings` passed. The continuity proof preserves the exact unfinished draft, one provider/owner and Graph progress; concurrent commands retain distinct repository traces and correlated default diagnostics. Gate owns the broader checks above; demo owns device judgment. The minimal contract and fixtures are shared with LOO-718.
