# Session steer subscriptions

Task `lf-9c03797`, 2026-10-11. Jack Heart accepted the Session-only primitive and
wrappers for pursuit (“ok, pursue”). After the stopped Flow reported the Codex
handoff and OpenCode boundary gaps, Jack Heart renewed pursuit (“pursuse!”).
Production cutover is not active. An October 11 implementation draft was withdrawn
after the configured Claude startup probe exposed a lock cycle. The accepted
outcome is unchanged; the startup authority revision below remains draft.

## Current implementation — reconciled 2026-10-11

At `03ba479ae2`, Task launch still reads event-backed steers into `TaskSeed`;
`TaskInput::poll` injects them on a five-second timer, and `RunningFlow` keeps
per-node cursors through `--steers-after`. `ContextDelivery::prepare` saves generated
Task summaries as references that later context hooks reread. No publication,
Session subscription, delivery receipt or `session_steers` integration target exists.
The data structures, commands and transaction below remain design, not shipped APIs.

Main's #1518/#1521 already supply checkout-based context refresh and a rendered
10,000-byte hook ceiling; extend that owner rather than adding another budget or
context reader. Main's #1524 preserves Codex live handoff while Claude/OpenCode
stop and resume native history. Neither change resolves callback authority.
Release child memory's operation-entry lesson applies here: helper tests cannot
establish public launch/hook dispatch or configured provider acceptance. Its
GOAL.md and complete MEMORY.md were reviewed; no deeper child scope was present.

## Startup fence counterexample — 2026-10-11

The proposed synchronous startup emitter cannot acquire the existing **exclusive**
Session attachment lock while the launching client holds it to send the first input.
Claude 2.1.296 runs `SessionStart` before consuming stream-json stdin. Under that
legal ordering, the hook waits for the client, the client waits for stdin drainage,
and `harness/dispatch.rs::write_fenced` reaches its two-second deadline.
`harness/claude.rs::send_input` then treats the partial seed as failed and tears down
the provider. Extending the timeout does not remove the cycle.

The [reproducer](claude-startup-fence.py) starts the installed Claude executable with
a disposable Home/config, synthetic fixture authentication and a loopback model
endpoint. Its startup hook takes the same OS `flock` exclusion as the proposed
emitter; the parent holds that exclusion while writing the first stream-json input.
The 131,133-byte input wrote only 512 bytes before two seconds elapsed. A separate
4,194,365-byte probe also wrote only 512 bytes. The hook marker appeared before any
stdin was sent. This proves the configured provider ordering and the proposed
lock cycle; it does not claim an existing production regression, successful model
delivery, or a complete public `lf` launch test.

**Draft authority revision:** attachment mutation keeps exclusive exclusion;
provider context output and native input dispatch need compatible read-side
exclusion, or an explicit startup protocol that completes before fenced input.
The read-side option needs an audit that each transport already serializes its own
writes, and proofs that takeover cannot cross either write. No choice is established
by the receipt tests. Simply emitting after dropping the fence is invalid: a
replacement could take authority before the output is exposed. Startup identity
activation also needs its own proof: Claude can replace the AgentProcess inside
`ensure_process`, after harness configuration was built; an adapter installed only
before `Harness::start` becomes stale on that path.

Dependent cutover stopped. Production Rust and tests were restored to `a839b8fad2`;
all old delivery paths remain together. The complete experimental diff is preserved
in [the compressed patch](session-steer-subscriptions.patch.gz), based on that commit.
It is unfinished evidence, not a shippable slice: Work publication wrappers,
source revision keys, native bootstrap, OpenCode adapter integration, prepared-crash
recovery, complete migration preservation and public adapter proofs are missing.
Four draft receipt tests passed (idle pending/one emission, uncertain write/explicit
retry, render failure, and provider handoff/replacement); they do not establish the
provider cutover. No migration touched the installed store.

The first slice now includes the startup exclusion/identity proof before old-path
removal. Its public fixture must force startup to overlap a first input larger than
the pipe buffer, preserve the complete seed, and test takeover during each side of
that exchange. The complete one-PR replacement remains required.

## What to build

Deliver authored repository, Wave, Task and direct Session direction through one
durable Session subscription, at its next supported provider boundary, with bounded
context and honest delivery evidence.

## Accepted scope

Infrastructure owns execution continuity and exact control authority. This is
separate from LOO-427's Desktop/CLI changes.

Jack Heart's October 11 accepted brief requires downward propagation, independent
Session delivery, launch Task context and idle staying idle. His current request:
“Replace the old injection paths together; make acknowledgement and provider
limitations concrete before implementation.”

The [accepted brief](../.lf/prompts/20261011-004454-11ad3078-2666-480c-aadd-d14954b69317-amVmgg-summary.md)
supersedes exact-scope-only direction. Jack Heart's subsequent review selected
Session-only delivery with repository and Work wrappers; no combined `SteerTarget`.

This is an indivisible replacement: internal slices, one PR. Delivery ownership,
launch seeding and old-path deletion cannot ship independently without two cursors.
No follow-up Tasks are proposed.

## Provider delivery authority — 2026-10-11

Codex live handoff outlives the launch claim. `store/sqlite/processes.rs::attach_in`
preserves `agent_process_id` when `replace_provider=false` but always creates a new
attachment token. `harness/codex.rs::start_inner` installs the hook configuration
on the provider thread. `harness/codex_connection.rs::serve` forwards reconnect's
`thread/resume` with only `threadId`, deliberately removing configuration overrides.

Counterexample: A launches a provider with an adapter tied to A's claim; B takes
over the same provider; a Task steer arrives; the next tool invokes the original
adapter. Comparing A's token with B's rejects delivery forever on that provider.
Blindly substituting the current token instead would give an old callback new
authority, including after A → B → A. Retiring live handoff is outside this Task
and is explicitly excluded by Infrastructure's AgentProcess direction.

Implementation choice: distinguish provider-originated context emission from
attached-client input. Save the existing AgentProcess ID and provider conversation
identity with the adapter. Under the Session attachment fence (whose startup exclusion mode remains unresolved
above), verify that
the same provider still belongs to the Session, capture the current claim, and
hold that lock through the bounded output write. Receipts retain both provider
identity and the claim observed for that attempt. Provider replacement rejects
the old adapter; attachment-only handoff preserves the provider's adapter.
This must not upgrade client commands, passive viewers or unrelated callbacks.
Bind the installed adapter to its recorded AgentProcess and native conversation
through a private launch-owned capability, scoped only to context delivery. Reuse
existing provider provenance where it establishes that binding; otherwise include
the capability in the Task's single migration. Do not derive it from the current
attachment token or accept an arbitrary caller's identities. Attachment-only
handoff retains the provider capability; provider replacement revokes it. Reject
child-conversation callbacks. Prove A → B → A handoff, replacement and stale
callbacks at the public hook boundary. No general process-control authority is
added. Explicit adapter rebinding is not a second implementation path.

Provider contract findings that survive either choice:

- Codex's published common `session_id` can name the parent for subagent hooks.
  The pinned [0.161.0 schema](https://github.com/openai/codex/blob/rust-v0.161.0/codex-rs/hooks/src/schema.rs)
  includes optional `agent_id` on `PostToolUse` and `UserPromptSubmit`; filtering
  solely by Session ID would consume the parent's receipts from a child. Pin a
  parent/child fixture using that field before enabling delivery.
- OpenCode [1.18.33's plugin contract](https://github.com/anomalyco/opencode/blob/v1.18.33/packages/plugin/src/index.ts)
  passes an empty input to `experimental.chat.messages.transform`, not a request
  ID. Its [prompt loop](https://github.com/anomalyco/opencode/blob/v1.18.33/packages/opencode/src/session/prompt.ts)
  creates the new assistant message separately from the supplied history, then
  calls the transform before `handle.process`. Its
  [processor](https://github.com/anomalyco/opencode/blob/v1.18.33/packages/opencode/src/session/processor.ts)
  retries the resulting stream input internally. A callback identifier cannot
  be labelled a provider request identity. Use a locally generated adapter invocation
  ID, explicitly named as such, for the delivery attempt. Session/publication
  receipts, not a supposed provider request ID, prevent reselection. Internal
  provider retries reuse the transformed input; a later callback does not append
  an emitted publication again. A lost transform remains emitted/unconfirmed under
  the reviewed retry policy. Pin that behavior in a protocol fixture; source
  inspection alone proves no configured delivery.

## The demo

Run two conversations on one Task, a sibling Task conversation, a parent Wave
conversation and a taskless Session. Keep one idle. Publish a repository steer,
a parent Wave steer, a Task steer and a direct Session steer. Continue through a
tool in each active conversation: each receives precisely its applicable direction,
with author and source. The idle conversation starts nothing; its next ordinary
continuation receives pending direction. Inspect per-Session evidence, then resume
and compact: neither operation continually replays old steers. Repeat with a
failed tool and an oversized Unicode message; complete source remains readable.

## Data structures and ownership

Use existing `RepoId`, `WaveId`, `TaskId`, `LfSessionId` and the Rust Session/Work
association resolver. No second hierarchy or Session registry.

- `SteerPublication`: stable UUID, source identity/revision, author,
  exact text and local acquisition sequence. This is the immutable published
  revision of direction; comments retain their mutable planning thread and sync
  receipts. Unique source/revision prevents reacquisition echoes. No timestamp
  cursor or equality-by-body deduplication. Source ownership stays with the
  existing repository, Work or Session record; it is not a delivery target enum.
- `SessionSteerSubscription`: one row keyed by Session, with association revision
  and activation evidence. Scope is derived, never another editable membership list.
- `SteerDelivery`: Session, publication, batch/attempt, provider conversation,
  attachment claim, boundary key, rendered payload hash and evidence. Unique
  `(Session, publication, attempt)`; explicit retry creates another attempt.

One migration draft against the released frontier adds publication and delivery
storage and converts historical Task/Project steer events to stable publications.
Keep event IDs and authored history through references to migrated publications;
remove duplicate event bodies/readers after consumer cutover. Unrelated event kinds
and Linear receipts survive. Historical records without a structured source retain
their complete text and deterministic origin key; do not parse attribution out of
prose or invent correspondence with a comment. New writers record source keys.

Task comment save/acquisition (`store/sqlite/task_comments.rs`,
`ops/linear_observe.rs`) publishes qualifying revisions in its existing transaction,
preserving outbound Linear receipts. Those receipts prove planning publication,
not provider-context acceptance. Definition edits acquired by
`store/sqlite/children.rs` also publish direction with stable source/revision keys.
A newer adopted comment revision supersedes pending older revisions from that
source, retaining both; already emitted revisions remain history and the correction
is new direction. Progress markers remain excluded.

## Publication and matching

The primitive is `steer_session(session, publication)`: idempotently make one
publication pending for one durable Session. Repository and Work commands are
thin wrappers over existing association resolution and this primitive. They save
direction with its source owner and resolve affected Sessions; they do not create
relay conversations or require an operator Session to propagate direction.
Preserve historical Project direction in reads; adding Project publication is
outside the accepted scope.

Proposed public commands extend existing owners: `lf comment <task> ... --steer`,
`lf wave comment <wave> ... --steer`, `lf repo comment ... --steer`, and
`lf session steer <session> ...`. Resolve names through existing selectors.
Return publication identity and saved status immediately; publishing starts no
turn. `lf session steers <session> --json` exposes pending/emitted/confirmed and
capability evidence; `lf session steers <session> --retry <publication>` requests
another delivery attempt, without waking the Session.

At launch and each boundary, the wrapper reconciles direction from the Session's
current applicable Task, Wave ancestors and repository through `steer_session`.
This catches new associations and interrupted publication fan-out without a worker.
Direct Session commands use the primitive immediately. Match publication IDs, so a steer
reached through multiple associations appears once. Nothing propagates upward or
sideways. Repository-less Sessions still receive direct direction. Unknown ancestry
is reported; it never broadens scope to all Work.

Reviewed backlog default: a new Session or newly added association receives existing
applicable, nonsuperseded direction through the same bounded seed. Existing receipts
remain valid; leaving and re-entering a scope does not replay it. Removed-scope,
not-yet-emitted batches are cancelled and recomputed. Emitted history never changes.
A new Flow-step Session gets its own bounded seed; repeating within the same
Session does not. No per-node suppression survives.

## Key functions and boundary transaction

```rust
steer_session(session: LfSessionId, publication: SteerPublicationId) -> Result<()>
steer_work(work: WorkRef, input: AuthoredSteer) -> Result<SteerPublication>
steer_repository(repo: RepoId, input: AuthoredSteer) -> Result<SteerPublication>
prepare_delivery(session: LfSessionId, boundary: ProviderBoundary)
    -> Result<Option<SteerBatch>>
record_delivery(batch: SteerBatchId, evidence: DeliveryEvidence) -> Result<()>
session_steers(session: LfSessionId) -> Result<SessionSteerStatus>
```

Direct Session publication saves its source on that Session. Repeated
`steer_session` calls preserve pending entries and receipts; only explicit retry
creates another attempt. Provider delivery takes a Session, never a Work target.

`prepare_delivery` serializes selection and reservation in SQLite, using the current
attachment fence and provider conversation identity. It reserves exact publication
IDs before rendering; concurrent callbacks cannot select them again. Duplicate
boundary callbacks return no second payload. Rendering/filesystem failure before
emission releases the reservation. Do not hold a transaction across provider I/O.

Launch prepares the same batch after allocating the durable Session and before
assembling top-level Task context. The launch batch and subsequent hooks therefore
agree on exactly what was supplied. Publications arriving after that transaction
remain pending. Split the saved Task brief from consumed steer bodies so reference
reload at compaction/resume cannot bypass receipts. Preserve the brief, workspace,
Project context, source attribution and the existing compact-only skill behavior.

## Acknowledgement and crashes

Evidence means exactly:

| Evidence | Meaning |
| --- | --- |
| Saved / pending | Publication exists; no emission attempt has begun |
| Prepared | Exact payload reserved; not yet exposed to the provider |
| Emission uncertain | Write was about to begin or was interrupted; receipt cannot establish acceptance |
| Emitted, unconfirmed | Hook output completed locally; provider acceptance remains unknown |
| Confirmed in context | An owned provider input/history record contains the batch ID and matching payload hash |

Mark emission uncertain before the external write, then emitted after successful
write. Crashing between these operations cannot be made exactly-once by SQLite.
Known pre-emission failures remain pending and retry at the next boundary. The
reviewed default for uncertain/emitted batches preserves evidence without automatic
replay; explicit retry is available. Unsupported adapters
must retain uncertainty rather than manufacture confirmation from exit zero, elapsed
time, another tool call or model-generated acknowledgement. Confirmed input never
means the model understood or followed direction.

Process replacement retains Session receipts. A stale attached client cannot emit
or advance them; the surviving provider's scoped adapter uses the authority above.
Reconcile prepared attempts using exact attempt/process evidence;
age alone does not authorize another emitter. No background polling, Stop-hook
continuation, auto-turn, replacement worker or terminal keystroke injection.

## Context bounds

Use one 10,000-byte rendered budget across Loopflow's additions at a boundary,
including escaping, attribution and source pointers. Reserve space for a short
steer notice before reference files; the active skill remains accessible as today.
Drain pending revisions in local acquisition order, in bounded batches. Cap inline
items at 32; oversized items get a marked UTF-8-safe excerpt and an immutable
complete source file. Materialize only the selected page, not the whole backlog.
Report remaining count and a paginated source command. Receipts identify exactly
which revisions were represented, including excerpt-only delivery; a pointer is
not evidence that the full source was read. New direction arriving during rendering
is eligible at the next boundary. Source-write failure consumes nothing.

## Provider boundaries and limits

Documentation inspected October 11; these are adapter targets, not configured proof.

| Provider | Proposed active boundary | Limit / evidence required |
| --- | --- | --- |
| Codex | Synchronous `PostToolUse` additional context; startup/resume/compact and ordinary user continuation share preparation | Documented tools include shell, edits and MCP; shell nonzero exit is covered. Prove nested code-mode visibility and actual enabled surface. Hook stdout alone is unconfirmed. |
| Claude | `PostToolUse` and `PostToolUseFailure` additional context; startup/resume/compact and ordinary user continuation | Failure hooks exclude pre-execution validation rejection and cancellation. Next supported boundary remains pending. Parallel callbacks share reservations. |
| OpenCode | Existing messages transform, before a model request after tools | Use an adapter invocation ID, not a provider request ID; receipt selection prevents reinjection. Pin retry/error behavior against 1.18.33. An ephemeral transform is not durable confirmation. |

Sources: [Codex hooks](https://learn.chatgpt.com/docs/hooks),
[Claude hooks](https://code.claude.com/docs/en/hooks),
[OpenCode plugins](https://opencode.ai/docs/plugins/).

Install only Loopflow-owned launch declarations, preserving user hooks/settings.
Extend Codex's existing exact-declaration trust hashing beyond SessionStart; no
global trust bypass. Providers may skip unavailable/untrusted hooks: status reports
the reason and pending count. Already-open providers without the adapter remain unavailable until a provider
start installs the declaration. Codex attachment-only reconnect strips configuration
overrides and cannot be treated as installation; no hot-reload or retroactive
installation claim.
Hooks validate their provider-bound capability and saved Session/AgentSession,
then observe the current attachment under the shared lock, not from cwd or
an arbitrary caller-supplied Session ID. Child provider conversations cannot consume
their parent's receipts.

Remote execution uses the destination Machine's existing command routing and local
Session store. Direct Session publication routes there; it is never planning data.
Work publications travel only through configured shared planning transport, retaining
source/revision identities; Session receipts and claims never synchronize. If transport
cannot exchange a scope, report unavailable exchange. No new cross-machine bus.

## Delete — do not maintain

Paths below are under `rust/loopflow/src/` unless stated otherwise. Delete in the
same cut as the replacement; do not refactor these predecessors separately.

- `RunningFlow.steers` and its node cursor logic in `lf/commands/flow.rs`;
  `Cli.steers_after`, forwarding/clone code in `lf/mod.rs`, and the seed filter in
  `lf/commands/run.rs` / `ops/task_input.rs`.
- `ops/child.rs::inject_live_steers`; TaskInput's steer cursor, seed-through receipt
  and five-second steer injection. Retain interrupt polling and attachment input;
  rename the remaining timer for interrupts. Preserve other `send_current` callers.
- Exclusive tests `store/mod.rs::live_steers_inject_new_comments_and_defer_when_not_steerable`
  and `rust/loopflow/tests/flow_tests.rs::a_repeated_node_receives_only_task_direction_newer_than_its_last_run`.
  Replace their behavioral coverage with independent Session receipt tests.
- Event-backed `task_steers`, `steers_since`, `append_steer` and
  `append_task_steer_in` in `store/sqlite/durable.rs` and `store/durable.rs` after
  publication migration. Cut comment/definition writers in
  `store/sqlite/{task_comments,children}.rs` and readers in
  `lf/commands/{activity,waves}.rs` over to the publication projection.
  Preserve progress exclusion, ordering, author/history and conflict tests.
- The inline author predicate in `store/sqlite/ci_incidents.rs::ci_incidents_since`
  also reads steer event bodies. Its human-assistance projection must join the
  publication after migration; `store/mod.rs::ci_incident_reports_find_human_help_in_task_comments`
  retains that behavior. Nested `ProjectEventKind::TaskObserved` copies of Task
  steer events need publication references too; changing only top-level events
  leaves duplicate bodies and incompatible historical JSON.
- Re-injection through saved Task summary references in `context_block.rs`;
  split the launch steer batch from the refreshed brief. Retain file refresh,
  compact-only skill context and native history.

## Forbidden outcomes

Two injection mechanisms; Flow/global consumption cursors; automatically waking
idle Sessions; applying sibling direction; promoting hook success to model-read
proof; replaying the full thread after compact; silently dropping oversized text;
mirrored Work ownership; using delivery to move Workflow or control a Process;
claiming remote exchange or already-open coverage from local fixture success.

## Internal slices

1. **This slice:** resolve the startup fence counterexample and establish/test provider-bound emission and OpenCode adapter
   invocation identity alongside the publication/receipt transaction and released-frontier
   migration; cut launch seeding over and delete old Flow/live-injection ownership
   together. Include minimal provider consumers so removal does not strand delivery.
2. Complete all provider boundary adapters, exact trust and status/retry surfaces;
   prove races, process replacement, backlog and bounded source access.
3. Exercise public CLI/provider fixtures; update `docs/architecture/planning.md`,
   `docs/architecture-reference.md` and steering/context command documentation.
   No separate PR or compatibility period between slices.

## Done when

Add a headless `session_steers` integration target. With inherited `LF_*` removed,
disposable stores and fixture `LF_BIN` pinned inside the fixture network boundary:

```sh
cargo test -p loopflow --test session_steers < /dev/null
cargo test -p loopflow --lib < /dev/null
cargo test -p loopflow --test flow_tests < /dev/null
cargo fmt --check
cargo clippy --all-targets -- -D warnings
```

Expected: scope/ancestor matrix, independent peers, taskless/direct delivery,
idle zero launches, launch races, concurrent/duplicate hooks, failed tools,
pre/post-write crashes, explicit retry, resume/compact, binding/ancestry changes,
source revision deduplication, Unicode/backlog bounds and released-frontier
preservation all pass. Interactive and headless protocol fixtures cover all three
adapters and unavailable/untrusted/open-process cases; remote fixtures preserve
the planning/execution separation. Add DTO fixtures in each consuming language
if the status shape crosses that boundary. Configured provider probes use isolated
homes and headless fake endpoints; unavailable probes are explicitly deferred to
capable CI, never recorded as passes.

Check: `uv run python scratch/claude-startup-fence.py`, 2026-10-11 — Claude 2.1.296 reproduced the startup lock cycle (512/131,133 bytes in two seconds); four archived receipt tests passed, production cutover withdrawn; `git diff --check` and `lf context` pass, provider acceptance remains unresolved.
