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

Implemented at `d168ef82e5`, then simplified at `b1029df2dc`, following the
November 3 concept review of Jack Heart's continuity requirement. The earlier
fixture dropped its viewer before restart and proved owner survival only; the
current fixture interrupts a connected relay. Losing the Machine connection keeps
the terminal relay and native surface alive. The relay reports “Machine connection
lost”; Ctrl+R deliberately reconnects and Ctrl+] closes. It retains the exact owner, reconnect token and
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

The real relay stays connected during the foreground service-restart proof,
returns to the unfinished draft and lets the same Graph advance. Additional proofs cover
another viewer taking input/geometry during the outage, a byte stream exceeding
the retained spool, and owner exit with no provider relaunch. Native presentation,
installed-update composition and usage accounting still belong to gate/demo below.
The review page at `scratch/concept-review.html` compares the earlier gap with
the current recovery contract. The recovery change needs no new product decision;
the terminal prompt and deliberate Ctrl+R interaction remain for native review.

### Earlier headless concept review · November 3, 2026

That review found no further design change or unresolved product choice in the
local slice. The interactive review below reopens recovery presentation; it
supersedes this earlier conclusion about having no open product choice.
Jack Heart's accepted experience remains: open a Task's recorded primary Session,
edit it, and recover access after a helper interruption without starting another
provider. Task → primary Session → existing owner maps directly to `view_plan`,
`session_owner` and `attach`; a service incarnation identifies a connection boundary,
not another Work lifecycle. The relay's output state is ephemeral; persistence,
execution and retention stay with their existing owners. No recovery registry,
new Session state or gateway command receipt is needed.

The normal path, service interruption and deliberate recovery now follow that
model. Preserve the counterexamples: another viewer taking control must leave the
returning viewer passive; expired output requires a fresh view, not input replay;
an ended owner must remain ended; a lost rename reply remains uncertain. The
existing service README already describes this experience, so no usage or skill
rewrite is needed. The remaining acceptance below is still required: especially
composed installed replacement, native continuity and unchanged usage accounting.
A failure there requires repair within this contract, not inferred approval to
restart providers or relax continuity. Remote admission, phone takeover and
unattended hosting remain later product choices outside this slice.

### Interactive concept review · November 3, 2026

Jack Heart requested concept-review and an additional review simulating Mitchell
Hashimoto's perspective. This is an analysis inspired by Hashimoto's public
writing, not his participation, endorsement or a prediction of his verdict.
The review is against `07a6a184d4`; no implementation or execution restart is
authorized by this review alone. The Task remains at `feature/demo`, with
PR #1813 published and Auto-Ship unapproved. No new native demo was performed.

**Accepted model remains:** a Task selects its recorded primary Session; the
existing execution owner retains provider, terminal and input claims. The Machine
service carries reads, complete CLI commands and attachments. Desktop owns native
presentation and UI automation. Keep the same owner through a connection failure,
never replay uncertain input, never take a newer viewer's control, and never start
an ended provider merely to restore a connection. The local scope excludes remote
admission, create/start recovery, unattended hosting and a second retention policy.

**The consequential mismatch:** a helper restart is currently a terminal
interaction. `transport.rs::reconnect_machine` writes the connection error into
the terminal, drains outage input and waits for Ctrl+R. Work subscriptions recover
through their existing observation path, while the terminal remains interrupted.
This is implemented behavior; rendered usability is still unverified. Deliberate
Ctrl+R was earlier derived implementation direction, not an explicit choice Jack
made between recovery interfaces.

**Proposal, awaiting Jack's judgment:** automatically reconnect the exact live
owner, show connection state outside terminal content, and keep input disabled
until the owner acknowledges the attachment and its current claims. Use bounded
retry/backoff for transient connection loss; an ended owner, disabled service or
incompatible installation remains an explicit state. Returning to another viewer's
Session stays passive. Stop retrying when the pane closes. No queued raw keys,
command resubmission, provider resume, or takeover is implied. A native Reconnect
button with deliberate retry is the alternative; retaining Ctrl+R is also valid
if Jack prefers the existing interaction. No option has been accepted yet.

The proposed client state is ephemeral: connected, reconnecting, or unavailable,
with input capability derived from the existing owner acknowledgement. It adds no
durable Session state or service registry. The current CLI relay exposes status
as terminal text; native presentation would need a structured connection-state
signal from that relay or a native bridge consumer. Keep that signal outside the
provider's byte stream. Choose the smallest implementation after the interaction
is selected; the public service operations need no new execution authority.

**Draft usage wording for the automatic option (unimplemented):**

> Open a Task to return to its Session. If the Machine connection drops, the pane
> shows “Reconnecting…” and pauses input. Once the same owner is reachable, its
> screen refreshes and input returns if control is free. If another viewer holds
> control, the pane stays read-only; use Move here to take control deliberately.
> An ended Session stays ended. Uncertain typing and commands are never resent.

Keep existing operator skill instructions unchanged: recovering a view grants no
permission to run a Task, resume a provider or approve shipping. The CLI's current
documented Ctrl+R path remains true until a selected change is implemented.

**Hashimoto-inspired assessment (our application of the sources):**

- Keep the reusable core small and exercised by real clients. Reusing Rust reads,
  CLI operations and the existing owner protocol is the strongest part of this
  design. The Swift fixtures are useful consumer feedback; they do not establish
  a live phone connection. Avoid widening the protocol before the local Desktop
  experience works. Source: [Libghostty Is Coming](https://mitchellh.com/writing/libghostty-is-coming).
- Make the next result tangible. Installed helper replacement with an editable
  draft, quit/reopen and continued Graph progress is the useful demonstration.
  Separate registrar and foreground-restart tests cannot establish the composed
  result. Preserve Linux and telemetry acceptance too; this priority does not
  waive them. Source: [Building Large Technical Projects](https://mitchellh.com/writing/building-large-technical-projects).
- Review how interruption feels. The terminal prompt is mechanically explicit,
  but exposes helper recovery to someone editing a Session and changes the screen
  being recovered. A native status surface could preserve context. Automatic retry
  versus deliberate retry is Jack's product choice, not a conclusion attributed
  to Hashimoto. Source: [You Have to Feel It](https://mitchellh.com/writing/feel-it).
- Exercise actual failure transitions. Retain the existing takeover, expired
  output and ended-owner fixtures. Add the composed update proof and lost-command
  reply/usage/cancellation evidence already required by acceptance; passing the
  happy path cannot settle these. Source: [Testing Error Recovery](https://mitchellh.com/writing/tripwire).

**Lifetime qualification:** the existing two-minute detached quiet grace excludes
held drafts, active turns, pending requests and running Graphs; headless Runs do
not expire for lack of viewers. The service does not promise indefinite live
retention of an empty idle prompt. An initial observation that an unviewed Session
can expire is incomplete without those holds. Keep the existing policy and prove
both held-work survival and honest ended-Session presentation. “Standalone” here
means service lifetime independent of the open UI, not an independently distributed
Mac host: the selected signed app still supplies its helper and registration.

**Open question:** automatic exact-owner reconnect with native status, deliberate
native Reconnect, or the existing Ctrl+R prompt? The question is in the current
conversation. `scratch/concept-review.html` presents the actions, models, API
boundary and alternatives. No response is not approval. After Jack answers,
update this design; implement only when requested, then verify the composed
installed path and record native feedback at demo.

### Review continuation · November 4, 2026

Jack Heart invoked concept-review again. Inspection of the current relay confirms
the existing Ctrl+R interaction; the recovery-interface question above remains
unanswered. Task status still places LOO-593 at `feature/demo`. This review changes
no implementation and supplies no new native or installed-update proof.

Keep **unreachable** distinct from **ended** in the proposed presentation. A
failed connection or refused attachment proves that this view cannot attach;
it does not by itself prove provider termination. Show “Session unavailable” when
the exact owner cannot be reached, preserving the last screen as stale. Show
“Session ended” only with authoritative execution evidence. Neither state permits
implicit provider restart. Disabled service and incompatible installation need
their own actionable explanations. This follows the accepted truthful-state
requirement and applies to all three recovery-interface options.

The proposed action/API sequence stays small: open Task → `read(view_plan)` →
recorded primary Session → `session_owner` → `attach` to that owner. On interruption,
recover only that attachment and its current claims; Work subscriptions separately
replace their readings. Native connection state is ephemeral client state, sent
outside terminal bytes. No SQLite migration, new Session lifecycle, provider-usage
event or durable request receipt follows from this interaction change.

Draft usage addition, **proposed**: “If the Session is unavailable, the pane keeps
its last screen and marks it stale. An unavailable connection does not mean the
Session ended. Reconnecting never starts another provider.” Existing operator
skills remain accurate and unchanged: restoring access does not run a Task,
resume a provider, take control or approve shipping.

Next proof after the interaction is selected: interrupt the installed helper while
editing a held draft; recover the same provider and Graph; repeat with another
viewer holding control and with an unreachable owner whose exit is unknown. The
last case must stay unavailable, not become “ended.” Preserve all remaining
acceptance below, including installed replacement and unchanged usage totals.

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

Checks: `cargo test -p loopflow --test transport_tests machine_service_` — 5 passed, 1 supervisor test deferred to gate; `cargo test -p loopflow --lib machine_service::` — 3 passed (macOS debug linker warned about unwind-table size); `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `git diff --check` — passed. Retained output state now resets together; direct/bridged viewer setup and wire round-trip checks share their existing fixtures. Earlier supervisor/Swift evidence: `d168ef82e570b8950721761f3b0170524298f7ac:scratch/mobile-backend.md`. Gate/demo retain the acceptance above, including the full transport suite and Linux Clippy.

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
