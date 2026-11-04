# One Machine backend for Desktop and phone

LOO-593 · Accepted local scope; implementation reconciled November 4, 2026 · Product

## Current implementation direction · November 4, 2026

Jack Heart requested addressing the preceding review before shipping: implement
automatic exact-owner reconnection with native connection status and prove
installed-helper replacement while an unfinished draft and Graph remain live.
This selects the review's recommended automatic option and supersedes its pending
choice and the earlier deliberate Ctrl+R direction for Desktop. Return to the
interactive demo with the changes published; no merge or Auto-Ship approval.

Keep connection status outside provider terminal bytes. Disable input during
recovery, use bounded retry/backoff, cancel on pane closure, preserve another
viewer's input and geometry claims, and never replay uncertain input or commands.
Unavailable is not ended: only authoritative execution evidence establishes exit.
Disabled or incompatible service states need an actionable native explanation.
The existing owner keeps lifetime policy and execution. Do not add a durable
viewer registry, Session lifecycle, scheduler or retention authority.

Implement and demonstrate the composed installed path in an isolated signed
installation: retain exact provider, Session owner, editable draft and live Graph;
replace the executable through the real installation/registration path; restore
the view; edit and submit the retained draft; verify continued Graph progress.
Also prove newer-viewer control, expired output, unavailable owner and cancellation
without input replay. Separate foreground restart and registrar tests are useful
but cannot substitute for this composition. Record native presentation and
Desktop quit/reopen evidence and preserve the existing telemetry/other acceptance
obligations below. Bring the installed-update proof forward into this revision,
rather than deferring it to the post-demo gate again.

LOO-718/PR #1812 now incorporates `07a6a184d4` and implements remote mobile
transport, pairing and client integration in its own checkout. Keep this revision
focused on the local backend and Desktop recovery. Inspect overlapping fixes in
LOO-718 before changing shared boundaries; preserve the published contract and
make any necessary additive consumer changes explicit. Do not import the wider
mobile feature set into this Task. LOO-593 remains the foundation to land first
after its checks and demo; the order recommendation itself approves no landing.

The local contract was published at `5b44a394cf` and consumed by LOO-718.
Native continuity review remains open.

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

### Automatic exact-owner recovery · November 4, 2026

Jack Heart selected this interaction after the November 3–4 review. Desktop now
owns a private temporary per-surface status mailbox. The relay sends atomic JSON
connection readings outside terminal bytes; native chrome shows reconnecting,
read-only or unavailable, and offers Retry after eight capped-backoff attempts.
The surface removes its mailbox on closure, cancelling recovery. No database,
Session state, viewer registry, provider launch or retention policy is added.
Only the existing owner’s exit event establishes ended; a failed bridge remains
unavailable with the retained screen marked stale.

Input waits for the exact owner’s acknowledgement. Recovery requests free claims
without replacement; a newer input/geometry controller remains authoritative.
Fresh terminal snapshots restore screen/parser state; byte streams resume their
received position, refreshing if the spool expired. Outage input and uncertain
writes are discarded. Retry is a mailbox signal, never provider input. Native
read-only mode preserves selection, copy, Find and scrolling. Plain CLI Machine
relays retain deliberate Ctrl+R; Desktop has no recovery text in provider output.

**Delete — do not maintain:** Desktop’s terminal recovery prompt and Ctrl+R
dependency, including `Status::closed_or_retry` interpreting raw keys as Retry;
the installed fixture’s kickstart-only replacement path. These are removed.
Native Retry reads only the mailbox; one relay owns ordinary mutable retry
counters. `MachineServiceProof::command` pins owner, Graph, relay and command
children to the installed executable under test. Preserve ordinary CLI
attachment, existing owner protocol, claim fencing, app-owned UI automation and
the shared mobile contract. LOO-718’s overlapping owner-start and program-status
changes were inspected; this revision neither imports them nor changes that wire.

The composed macOS fixture installs an actual lf helper in an isolated signed app,
starts the owner and Graph from that installed path, and holds a draft. It replaces
the executable inode with identically signed bytes, verifies the bundle seal and
invokes the production registrar’s unregister/register path. LaunchServices
registers the temporary app container and launches its updater as the app;
macOS App Management correctly refuses an external test runner’s bundle writes.
Connection generations prevent a stale connected reading
from satisfying recovery. It then edits/submits the draft and checks exact
owner/provider identity and Graph progression. This is executable replacement
with the same candidate build, not a released-version migration or a rendered
Desktop demonstration.

Historical review, source links and rejected alternatives remain at
`82fdb0e928:scratch/mobile-backend.md`; Jack Heart’s accepted November 4 direction
leads this document. Review feedback also required local reading interactions and
installed-owner executable retirement, included above.

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
insufficient evidence. The composed signed-container fixture replaces the
installed executable while its owner and Graph remain live. Linux uses a user
service with normal control-group cleanup: this slice only renames Sessions and
bridges existing owners. Execution-start operations need an isolation proof
before entering that service.

Respect disabled Background Items rather than registering around them. Personal
LaunchAgents end at logout; sleep makes the host unavailable, and reboot preserves
records rather than live processes. Before-login hosting is a separate promise.
Removal must preserve Work and credentials and define active-session handling before
implementation. Deleting Desktop cannot imply supported standalone hosting.

## Remaining acceptance

Gate owns affected suites, the Xcode link path, Linux user-service replacement,
subscription saturation/cancellation and unchanged provider-usage totals across
reconnect. Attribution/default equivalence beyond the rename result remains open.
Demo owns actual Desktop quit/reopen, consent/disablement, native input and rendered
editable Session continuity. The installed headless fixture proves signed
registration, executable replacement, a held draft, exact owner/provider identity
and Graph progress. It does not prove native input, phone interaction or
real-provider usage reporting. Terminal slow-consumer bounds, cancellation and
command trace isolation are covered.

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

Checks: `cargo test -p loopflow --test transport_tests machine_service_ -- --include-ignored --test-threads=1` — 9 passed; the installed test binary’s exact replacement case also passed after adding concurrent setup and the LaunchServices-owned updater. `scripts/test_desktop.sh --filter MachineConnectionTests -Xswiftc -gnone` — 1 passed headlessly, app built; formatting, Python lint and diff checks passed. Gate retains affected suites, Xcode/Linux paths and telemetry; demo retains rendered continuity and Desktop quit/reopen.

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
