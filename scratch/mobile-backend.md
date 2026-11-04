# One Machine backend for Desktop and phone

LOO-593 · Accepted local scope; implementation reconciled November 3, 2026 · Product

Jack Heart authorized the installed local Machine backend through launch-plan,
continuing to interactive demo without Auto-Ship approval. LOO-718 builds the
native iOS client in parallel. The local contract was published at `5b44a394cf`
and consumed by LOO-718, now at interactive demo with PR #1812. Native continuity
review remains open.

Jack selected native Swift, host Tailscale, terminal Sessions, CLI command
delegation and the describe-first new-Task sheet. Local revision 1 implements
neither remote admission nor recoverable create/start. The original HTML review
in LOO-718 contains recommendations, not additional accepted decisions. This is
the single active backend design.

## Local slice and ownership

```text
Desktop ── private Unix socket ── Machine service
                                   ├── shared Rust Read / Subscription
                                   ├── complete command via installed lf
                                   └── bridge to existing Session owner ── agent / PTY

Desktop owns UI automation and native terminal rendering.
The existing Graph runner and Session owners retain execution and lifetime.
```

Desktop now sends roadmap reads/subscriptions and Session rename through the
service, with no direct-FFI fallback. Session panes bridge their exact existing
owner. Other Work operations retain their current transport until the full
cutover in LOO-718’s task plan. Preserve RegistryQuery consumers and domain DTOs.
The app-owned DesktopServer still lives and dies with Desktop and owns no Sessions.
Its original routing/discovery questions remain in
[the original scope](research_notes/desktop-server-original-scope.md).

The service runs as the existing OS account and data directory. It owns admission,
framing and bounded delivery; it introduces no scheduler, PTY manager, retention
policy, shadow store or Task orchestration. Shared CLI operations own resolution,
defaults, mutation, attribution and recovery. CLI use need not depend on the
service. Window focus, pane arrangement, menus and renderer FFI stay in Desktop.

**Delete — do not maintain:** the migrated roadmap read/watch and Session-rename
FFI closures are removed. The service’s duplicate Checkpoint DTO/parser is removed;
Subscription owns checkpoint validation, argument decoding and the final error
reading. No exclusive fixture depended on that DTO. Machine bridge loss no longer
exits the live relay and discards its connection state; direct CLI attachment
retains its existing behavior. The full-cutover follow-up
must inventory and remove remaining LoopflowLibrary/ServedWatch Work glue and
unused lf_read/lf_write/lf_subscribe exports with exclusive build/test wiring.
Move shared Rust implementation rather than copying it. Preserve renderer FFI,
CLI operations, SSH and app-owned UI automation. Do not restore lfd or a fallback
for migrated operations.

## Protocol and reconnection

[rust/loopflow/src/machine_service/README.md](../rust/loopflow/src/machine_service/README.md)
owns revision-1 operations and limits. Rust wire types, their Swift mirror and
`tests/fixtures/dto/machine/` are the shared contract already supplied to LOO-718.
LOO-718 identified that roadmap membership cannot select a primary Session. The
November 3 reconciliation exposes the existing `Read::ViewPlan` as `view_plan`,
with `tests/fixtures/dto/machine/view_plan.json`; it returns authoritative
`contents[].primary_session` before `session_owner` resolves the existing owner.
The additive capability and fixture were published on the Task branch at
`12fbe9102c` for the parallel client.
No selection heuristic or new DTO is introduced.

One operation uses one same-user Unix connection with length-prefixed JSON.
Requests have correlation IDs; replies include service-instance identity. Bootstrap
advertises version, executable identity and capabilities. Required/optional fields
have no implicit DTO defaults.

- Reads use existing Rust Read::read. Roadmap subscriptions use Read::subscribe,
  complete replacement readings, generation and explicit unavailable state.
  Blocking reads run off network workers; closing cancels. The shared watch owns
  checkpoint errors and coalescing. Bounded consumers reconnect for fresh state;
  store revisions are not replay cursors.
- Session rename delegates Write::argv to a real lf subprocess. Each command has
  an isolated environment and fresh CLI trace. No process-global caller mutation
  or service→CLI→service recursion. Request IDs provide correlation only. A lost
  reply means outcome unknown; never automatically resubmit it.
- Session-owner lookup never launches or resumes a provider. Attachment bridges
  the existing binary owner protocol with bounded buffers. The owner retains
  input/geometry claims, stale-control rejection and output continuation. A missing
  owner fails explicitly; it never creates a replacement conversation. Ordinary
  CLI attachment remains direct; Desktop selects the bridge.

Reconnect takes fresh Work readings and resumes bounded terminal output or replaces
its screen. It cannot steal a newer controller claim or replay uncertain raw keys
or Send. Voice drafts remain on the phone until deliberate submission.

### Connection recovery is not Session resume

Implemented following the November 3 concept review of Jack Heart's continuity
requirement. Losing the Machine connection keeps the terminal relay and native
surface alive. The relay reports “Machine connection lost”; Ctrl+R deliberately
reconnects and Ctrl+] closes. It retains the exact owner, reconnect token and
received-byte position in memory. There is no persisted viewer registry or new
Session state. Desktop never enters ordinary `lf resume --prepare` merely because
the bridge closed. Explicit Open and Move here keep their existing meanings.

Terminal recovery requests a fresh screen from that same owner: the local prompt
has changed the displayed screen, so replay alone would leave its cursor and modes
wrong. Byte-stream relays use the existing owner Resume protocol; expired output
falls back to a fresh attachment. Recovery requests free input without replacement
and queues geometry behind an existing owner. When another viewer has control,
it reconnects passively. Replayed claim notices cannot override the acknowledgement's
current claims or run a historical provider-successor command.

Typing while disconnected or awaiting acknowledgement is discarded. A failed input
write is uncertain and never retried. Service unavailability leaves deliberate
retry available; a service refusal naming an unavailable exact owner ends this
path explicitly without Session preparation or another provider. The owner retains
its existing lifetime policy. Work subscriptions separately reopen for complete
current readings; uncertain rename outcomes are inspected, never resubmitted.

The real relay stays connected during the service-restart proof, returns to the
unfinished draft and lets the same Graph advance. Additional focused proofs cover
another viewer taking input/geometry during the outage, a byte stream exceeding
the retained spool, and owner exit with no provider relaunch. Native presentation,
installed-update composition and usage accounting still belong to gate/demo below.
The historical review page is `scratch/concept-review.html`.

## Installation and lifetime

The selected installation and public lf entry own service setup for Desktop and
CLI. Default-Machine commands delegate to the promoted CLI. Desktop automatically
obtains the service and checks readiness; disabled or incompatible service states
remain visible. Normal local use requires no Tailscale.

On macOS, SMAppService registers the selected signed app’s existing lf helper and
bundled LaunchAgent plist. A temporary ad-hoc-signed container proved the production
registrar. A separately distributed GUI-less signed container remains an option,
not a supported standalone distribution. Explicit development LF_HOME values use
separate supervisor identities; source builds cannot replace the default service.

Executable replacement may require unregister/re-register, terminating the helper.
Graph and Session owners must survive actual supervisor cleanup; setsid alone is
insufficient evidence. The isolated launchd fixture uses `kickstart -k` and proves
service restart continuity for existing owners. The signed-container fixture
separately proves unregister/register, but neither replaces the installed executable
while Work runs.
That composed installed-update acceptance remains outstanding. Linux uses a user
service with normal control-group cleanup: this slice only renames Sessions and
bridges existing owners. Execution-start
operations need an isolation proof before entering that service.

Respect disabled Background Items rather than registering around them. Personal
LaunchAgents end at logout; sleep makes the host unavailable, and reboot preserves
records rather than live processes. Before-login hosting is a separate promise.
Removal must preserve Work and credentials and define active-session handling before
implementation. Deleting Desktop cannot imply supported standalone hosting.

## Remaining acceptance

Gate owns affected suites, the Xcode link path, Linux user-service replacement,
and composed installed-executable replacement with active Work.
Demo owns actual Desktop quit/reopen, consent/disablement and rendered editable
Session continuity. The headless owner fixture proves an unfinished draft, exact
provider/owner identity and Graph progress; it does not prove native input, phone
interaction or real-provider usage reporting. The signed-container registrar proof
and isolated launchd owner/Graph proof are distinct. Slow-consumer bounds and
cancellation are implemented; dedicated saturation/cancellation acceptance remains
with gate. Command trace isolation is covered; no fixture yet measures unchanged
provider-usage totals across reconnect. That telemetry acceptance remains open,
including attribution/default equivalence beyond the rename result comparison.

Preserve these obligations:

- CLI/socket equivalence in state, defaults and attribution; initial/change/error/
  cancel subscriptions; bounded slow clients and fresh reconnect readings.
- Concurrent requests remain distinct; concurrent startup obtains one service.
  Installed helper replacement and Desktop quit preserve existing execution.
  Disabled consent and version skew have truthful failure states.
- Telemetry explains command outcome or uncertainty without duplicating provider
  usage after reconnect. Commands retain shared Process records; observations add
  neither Work nor usage. Correlate request, service instance, child PID, trace and
  resulting Work identity without inventing execution ancestry. Diagnostics remain
  bounded; do not persist every snapshot or terminal byte. Exact additional
  latency/queue/reconnect metrics and retention remain implementation choices.

Checks: `cargo test -p loopflow --test transport_tests machine_service_reconnect_` — 3 passed; `cargo test -p loopflow --test transport_tests machine_service_refreshes_work_and_preserves_owner_and_graph_across_restart` — 1 passed with the real relay attached; shared-contract checks from the preceding pass remain applicable (`machine_service::` 3 passed; Swift `MachineProtocolTests` 4 passed). `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `git diff --check` passed; the expired-output proof passed again after clearing an obsolete reconnect token on fresh attachment. Gate/demo own the acceptance above, including the complete transport suite and Linux Clippy. Earlier supervisor/Swift evidence: `5b44a394cff91d0aa20c614c3b20c3f7e121affb:scratch/mobile-backend.md`.

## Later work and parallel boundaries

LOO-718 owns iOS targets, renderer/input integration, selected-Task UI and client
tests. LOO-593 owns Rust service/wire truth and representative macOS integration.
Coordinate shared Swift package/project edits rather than defining another wire
shape. Terminal recordings and fixtures support client work independently of
packaging. Live phone integration joins the paths; full Work cutover is required
before treating the wider architecture as supported.

Remote carrier, pairing/compatibility, phone takeover gesture and unattended Mac
hosting remain open. Recommend separately enabled HTTPS/WSS over host Tailscale
Serve with revocable credentials and expected Machine identity. Same-user socket
trust cannot authenticate a remote proxy; reachability is insufficient. Preserve
unrelated Serve routes. A future optional hub may own authenticated durable ingress
and retransmission, while Machine admission/execution stays shared and private use
never requires the hub. Existing explicit SSH command destinations remain valid.

Recoverable create/start belongs at the shared command boundary, not a gateway
response cache. Retain request identity and exact payload across external creation;
recover the original Task after launch failure, or report uncertainty. The eventual
sheet shows its last valid repository/Machine default and permits changes. Match
Desktop’s complete task-create/run operation and conversation admission; do not
substitute conversation-only launch, headless/ship approval or silent Machine
rental. Proof must lose a reply and retry without duplicate creation, preserve a
created Task after Graph failure, and show remote failure causes no local mutation.

Integrated at `324a0f6dc4`: LOO-720/#1809 terminal activity, including owner-sampled
status and output ordering; the bridge retains that unchanged owner protocol.
Preserve other overlapping work: LOO-582/#1807 Session identity; LOO-673/#1791
Desktop queries; LOO-312/#1656 installation; LOO-714/#1805 defaults; #1808 Wave DTO
revert (Task attribution unresolved). Recheck landed status when integrating. LOO-717 owns shell policy; LOO-709/710/712 own retention, snapshots
and Session transport. This service bridges those owners.

## Evidence

[Earlier lfd attempts](<reports/Lessons from earlier lfd attempts.md>) show that
shared executors still suffered orchestration drift (#304), exec delegation never
finished its cutover (#825), and shared functions could retain competing lifecycle
owners. No inspected evidence proves a completed thin backend was tried and failed.
[Installation research](<reports/Machine service installation and lifecycle.md>)
distinguishes observed supervisor behavior from product claims. Source research is
preserved at `0bd889e4b860fd3d7c44389a86914317c0d22af6:scratch/`; historical timeout
values do not override current owner policy (two minutes of detached quiet by default).
