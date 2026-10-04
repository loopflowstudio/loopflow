# Task conversation and ordinary background Flows

October 4, 2026. LOO-353, Product. **Draft implementation plan for review-design.**
Jack Heart accepted the interaction direction below and requested kickoff only.
Mechanisms and timeout proposed here are not implementation approval.

## Outcome and demo

Jack keeps designing, discussing and reviewing in one ongoing Task conversation.
Headless work appears separately as Runs. Returning to a Task preserves the same
conversation, files, drafts, shells and layout; understanding background work does
not require finding a privileged Flow or opening a new review conversation.

Demo: in a Task conversation, launch `lf -b flow <template>`, then a second Flow.
Both appear with the same graph, step, output and history affordances. Continue
working in another Task. When the first reaches review, its pending review appears
on the original conversation without stealing focus. Return, discuss the captured
review skill in that conversation, and give actionable feedback in ordinary prose. Only its
exact occurrence returns feedback to the next authored step. The other Flow and conversation
remain open. Kill a background runner: its evidence remains, it does not restart,
and the conversation can inspect the uncertainty before launching replacement work.
These are proposed commands and behavior, not a claim of a working demo.

## Accepted decisions

- All interactive Task design/review stays in the ongoing Task conversation.
  Deliberately opened additional conversations are allowed. Headless work uses
  separate Sessions through ordinary `lf -b`; discourage substantial inline
  implementation, while allowing incidental edits.
- Product calls interactive AgentSessions **Sessions**, headless AgentSessions
  **Runs**. Identity, native history and attribution survive mode changes. Exec
  remains an actual process; FlowSession remains an invocation, not a new Run type.
- **Waiting** is the one attention state. `--waiting` replaces `--needs-me`;
  `--interactive` selects mode independently. Provider-specific signals and false
  positives are acceptable. New activity clears heuristic Waiting. Claude uses
  the binary's stream-json path only: no SDK, hooks or permission host.
- Remove singular managed-Flow authority and mutable running-Flow switching.
  Caller owns crash recovery, normally the Task conversation. Retain effect and
  history evidence; no automatic database-backed Flow resumption is required.
- Jack's additional October 4 direction deletes `lf session ready`, `lf session
  complete` and their review handshake. Conversational feedback supplies the exact
  interactive boundary result without completing the conversation. No renamed
  finish/approve command or button replaces those APIs.
- Membership remains owning Home plus resolved checkout and explicit bindings,
  excluding explicit repository/Wave scopes. It does not follow attention, mode,
  primary selection or Flow participation. Selecting/hiding is not termination,
  review completion or authorization.

October 4 supersedes the older attention prohibition and switch-now/finish-then-
switch proposals. The supplied current Task definition supersedes the older
LOO-364 ownership split for this Task-conversation/runtime cut. Project independence
stays LOO-366; admission/completion policy stays LOO-367.

## Current system and reconciliation

Inspected this checkout at `2f14e44224cac531388712c088b03d32ce1a6adf` and local main
read-only at `c5dc238b0afb53dbe7098f30e2dee8085b53e1e3`. This is a local-main
snapshot, not a fetched remote tip. Existing scratch was preserved in
`6513477a5`; `task-workspace-continuation.md` remains unchanged historical evidence.
Its worker-repair/resume instruction is superseded, not an implementation task.
No sync, worker start, provider launch or live migration was performed.

Main already has repo/Wave primaries, Ask removal, provider-native conversation
lookup/connect, Session working-set filtering, configurable New Session, and Task
completion-history filtering. Preserve these cuts rather than recreating their
older counterparts. `PrimaryScope` currently has Repository and Wave only; Task
primary selection is not delivered by that implementation. Main still has:

| Owner | Observed responsibility and required change |
| --- | --- |
| `durable.rs::FlowSession` | Capture plus mutable cursor, version, attempt, pending Session and worker claim. Split live execution state from retained invocation evidence. |
| `store/sqlite/flows.rs` | `tasks.current_invocation_id`, `start_task_flow`, claim/reclaim, checkpoint/retry, review settlement. Delete runtime controller writes; preserve capture and event history. |
| `lf/commands/flow.rs` | One driver with managed branches, saved resume, child launch and checkpoint. Keep the engine/graph traversal; use an in-memory execution position. |
| `ops/flow_session.rs` | `reserve` creates a new `FlowReview`; `complete` closes it and launches a resumed driver. Replace with exact review input/feedback on the existing conversation. |
| `ops/human_session/primary.rs` | Scope admission, retained native identity, launch lock. Extend selection to Tasks without excluding Task primaries from membership. |
| `store/sqlite/flow_inventory.rs` | Indexed inventory and lazy detail already exist; managed bit/filter derives from the Task pointer. Reuse inventory/detail, remove privilege. |
| `TaskWorkView.swift`, `TaskFlowView.swift` | All-work rows versus richer selected-Flow graph. One detail view per selected invocation; no selected invocation acquires execution authority. |
| `harness/claude.rs` | Persistent binary stream-json stdin and retained provider ID already support successive inputs. Native Task conversation delivery is a separate missing integration. |

`lf/mod.rs` on inspected main exposes `--mode batch`, not a parsed `-b` option;
`ops/flow_run.rs::exec_driver` nevertheless emits `-b`. Treat that as a concrete
surface inconsistency. Add `-b` as the ordinary detached headless launch option
for skills, prompts and Flows, with one common placement/Exec path. Batch mode
selects provider behavior; detachment additionally returns a durable invocation or
Session identity after admission. Do not fix it by preserving the Task worker.

Read [the workspace review](../docs/reviews/task-workspace.md) and Unit 3 at
`bc78c27c017bc93099c06fd342b51bc6110beb5d:scratch/growth-thoughts.md`.
The review predates #1369's merge and main's Ask deletion. Its remaining native,
remote, file and performance proof survives; its old Ask and mutable switching
requirements do not. PR #1313/tag `backup/file-browser-20260930` is historical
web-prototype evidence, not a second file-editor implementation to restore.

## Chosen architecture — proposal

### One live runner; SQLite retains observations

Keep `FlowSession` as the immutable invocation header: ID, captured graph and
source provenance, cwd/Home/Work, launch options, caller Session and owning Exec.
The ordinary `lf flow` process owns `ExecutionCursor`, nested return counters,
selected attempt and exact awaited review in memory. Every authored agent step
launches ordinary headless work; mechanical steps remain Execs. A Flow containing
only commands does not gain an artificial AgentSession. `-b` detaches this same
runner; it does not select another executor.

Retain SQLite `flow_sessions` and `flow_events`, reshaped as history. Append started,
step-started, consumed successful Session-event references, mechanical outcome,
review-requested, feedback-consumed and terminal outcome evidence. Record node ID,
full nested occurrence/return tuple, Exec and Session IDs, timestamps and sequence.
Retain the captured graph even after the source changes or disappears. Existing
Session events own provider output, input, usage and feedback; Flow events reference
them rather than copying a second transcript. Last-observed position is a derived
read model, never a restart instruction. Indexed summary columns may be updated
transactionally with their event for cheap inventory reads, not independently.

The runner retains the existing process/driver lock for its lifetime. Remove worker
claim generations and takeover/reclaim; preserve Session driver/provider fencing,
Exec process evidence and native client ownership. No API accepts an observation
row as authority to move the cursor. Reads cannot start a driver. `flow resume`,
Task retry/restart/switch and automatic resume branches leave the runtime surface.
Stopping an exact process remains ordinary process control, not Flow editing.

History has no new automatic pruning policy. Page inventory and output through
existing readers; load one selected Flow's capture/details on demand. Session IDs,
effect receipts and historical step evidence survive Task completion and schema
conversion. Retaining tables for observation is intentional; deleting all Flow
storage would require rebuilding the graph and effect history elsewhere.

### Review handoff: suspend the runner, use the same conversation

**Proposal for Jack:** keep the ordinary runner sleeping at the review boundary.
No provider remains busy waiting solely for feedback. The runner retains its in-
memory continuation until feedback or process exit; no recovery supervisor exists.
This preserves one Flow identity and its authored nested loops across normal review.

At launch, capture the interactive caller's Session as review recipient. When a
Task Flow is launched outside an interactive Session, select that Task's primary.
Explicit New conversation remains independent; a Flow launched there returns there.
For taskless callers, use the calling conversation. Without a recipient, a Flow
containing interactive nodes fails admission with a useful explanation before
running effects; it does not invent a new review Session. Pure headless Flows need
no recipient. Selection is pinned to the Session, never whichever pane is focused.

At a human node:

1. Append a review request referencing `(Flow ID, node ID, nested occurrence,
   owning Exec)` and the exact captured skill/context. Link it to the recipient
   Session's existing input/history owner. Do not run the review skill headlessly
   first and then spawn an interactive replacement.
2. The existing conversation driver takes this input at an idle turn boundary.
   If busy, retain it for the next turn; preserve unsent composer text and never
   paste into a PTY. Multiple reviews remain individually selectable in that same
   conversation. Display availability immediately; availability is not completion.
3. Run the captured design/review skill as a turn of that conversation, preserving
   provider identity and prior conversation. Discussion can span multiple turns.
   Conversation events carry exact review references; the Session row cannot
   carry a singular `flow_session_id` as its complete participation model.
4. Jack gives feedback in the conversation: for example, “Use the second layout;
   keep the sidebar collapsed.” The review turn returns that feedback as its
   ordinary structured skill result when it answers the captured review, with
   references to the actual authored input events. Extend the existing
   `engine/flow_output.rs::FlowOutput` contract with a review-feedback value:
   exact request/occurrence, source input event IDs, summary and artifact paths.
   The driver binds the request from the captured turn context, not model-supplied
   routing. Save the result in the existing successful Session output/completion
   events; the Flow consumes that exact event. There is no ready state, second
   confirmation, completion tool call or Complete button. Ordinary questions and
   unfinished discussion produce ordinary replies with no boundary result.
   An ambiguous “yes” with two unselected reviews remains discussion; the agent
   clarifies inside the same conversation rather than choosing a Flow. Identical
   event replay is idempotent; stale or mismatched results cannot answer a later
   occurrence. Missing source feedback cannot be synthesized as Jack's approval.
5. The sleeping runner consumes that reference once and advances to the next
   authored step. In the required baseline, a following `loop-decide` still chooses
   Advance/Iterate from this exact feedback. Its interpretation cannot invent
   authorization or replace an unresolved question with approval.
   Consuming review feedback neither completes the Task conversation nor rewrites
   the Flow. A provider turn ending without a valid boundary result, Waiting,
   pane close and silence cannot do this.

Feedback delivery is narrow input to a live authored boundary, not mutable Flow
control. It cannot skip nodes, change a graph, retry effects or resume a dead runner.
This reuses the existing successful-step output mechanism, not a new callable
feedback API. A review result carries criticism/direction as well as acceptance;
the next authored decider owns navigation. Where navigation needs permission,
the original authored feedback must supply it. Mere readiness grants none.
The runner can wait on the existing store/read notification path; a bounded one-
second fallback poll reads only its exact request, without provider calls. There
is no daemon and no per-Flow automatic relaunch job.

**Integration work, not an existing capability:** `serve_conversation` still
launches a native conversation; headless harness `send_input` alone does not prove
native input delivery. Add pending review input to the existing Session event/input
owner and drain it only through that Session’s authoritative driver. Store append, delivery
receipt and provider acknowledgment must be correlated. On uncertain delivery,
retain uncertainty and inspect provider history before retransmission. A second
Harness, native keystroke injection, or generic message/outbox service is forbidden.

For Claude the supported mechanism is the existing persistent binary stream-json
input/output driver with the same native resume ID. Its interactive surface must
submit ordinary user turns and review turns through that one owner. The provider
TUI cannot be assumed to accept an injected review. This may require adapting the
existing conversation surface for Claude; do not silently replace the retained
native terminal experience. Review-design must settle that visible tradeoff before
implementation if the existing surface cannot meet both requirements. Codex/OpenCode
must likewise use their actual conversation owner, never a parallel provider process.
The [Claude CLI reference](https://code.claude.com/docs/en/cli-reference) documents
stream-json input/output; this supports the binary choice but does not prove the
installed version, native composer retention or this end-to-end handoff.

### Failure and recovery

Known runner exit leaves a stopped/failed invocation and unresolved effects visible.
Missing remote process evidence is Unknown, not Completed. Loss of a conversation
client leaves the same review pending while the runner is alive; reopening resumes
the conversation, not the Flow. App/window exit does not implicitly end the ordinary
detached runner. If the runner dies, review notes remain readable but cannot advance
it. A request accepted just before a crash can have saved feedback without a consumed
receipt; show those separately. Never infer that the next side effect did not occur.

Caller recovery inspects captured history, exact successful agent completions and
external-effect receipts, then launches a fresh ordinary invocation with explicit
scope. No `--from-cursor` or serialized continuation token. The caller may author
remaining work as an ordinary Flow; the old invocation stays stopped and linked as
context, not imported as successful steps of the new one. PID existence cannot
certify provider progress, and uncertain old children cannot be blindly duplicated.
Headless failures return through their existing output; the Task conversation reads
them through ordinary inspection. Do not add an escalation conversation or queue.

### Primary selection and Waiting

Extend existing primary selection with `PrimaryScope::Task(TaskId)`. Ensure under
the existing scope lock; reuse an explicitly chosen existing Task conversation or,
on initial selection, the sole unfinished interactive Task conversation. If several
exist and none is selected, default to the most recently used interactive conversation
and preserve the rest. Record that deterministic default as a proposal, not evidence
of Jack's preference. If none exists, explicit Task conversation opening creates one
in the recorded checkout; read-only inventory never does. A Task primary is still a
Task member: narrow current primary exclusions to Repository/Wave. No Task Ctrl-C
reset behavior is introduced; ordinary interruption never restarts its Flow.

**Proposed Waiting timeout: two minutes**, a single internal default, no settings UI.
Rust owns the projection; CLI, Desktop and other DTO readers consume it. Replace
Review/Reply attention presentation with one optional Waiting value, keeping lifecycle
and mode separate. Explicit pending input and a successful interactive yield with no
outstanding tools can mark Waiting immediately. Otherwise, after 120 seconds without
new provider turn/item/output activity and with zero unresolved tool calls, mark
Waiting. A launched conversation with no event yet stays opening/unknown rather than
being classified from its creation timestamp. Failed/closed Sessions retain their
lifecycle; a completed headless Run does not become a waiting conversation.

Correlate tool starts/results by provider turn and item ID, tolerate duplicate/out-
of-order events, and discard prior-turn outstanding sets only on known terminal
turn evidence. New activity clears heuristic Waiting; an explicitly unresolved
input can still require attention while another tool runs. Disconnection with
incomplete evidence stays unknown; do not pretend missing events prove no tools.
Use event sequence and monotonic elapsed time while live; imported history must
not reset a timer merely because it was reread. Native histories can supply weaker
provider-specific observations; no semantic question classifier is required.

`session list --waiting` intersects the selected mode/history filters before paging.
Default navigation retains unfinished interactive Sessions, as main now does;
Runs remain available in the Task's background work group. Put Waiting first and
working Sessions in a compact expandable group. Existing visible/focused panes
remain visible even when attention changes. Filtered absence is never deletion.
Mode changes update labels without changing identity, ownership or attribution.
Pending review metadata remains inspectable even while its preparation turn works.

### Other Session completion uses

Main's `ops/human_session.rs::complete` also stops an ordinary conversation's
provider and stamps `completed_at`; it is not exclusively a review API. Delete
that public operation too. Existing process-stop/interrupt controls continue to
stop exact clients without completing a conversation. Close view remains reversible
presentation. Conversations remain reopenable; no replacement `finish`, `resolve`
or `archive-and-stop` action is introduced. Primary replacement keeps its own
explicit successor/history semantics and cannot release any Flow review.

Preserve historical `completed_at` and feedback in migrated history so completed
records do not suddenly reappear as active work. Remove `ready_summary` as a live
column after copying its historical content into Session evidence. Remove Ready
from live Session lifecycle/action projection; successful provider turn completion
and Run outcome remain execution facts. Inventory and cleanup must no longer rely
on people completing conversations: stopped clients remain accessible in the
existing retained/history presentation, and closing their pane does not erase them.
Whether a later presentation-only archive affordance is useful is outside this
cut, not a reason to preserve the completion API. Task completion still belongs
to its existing Task operation, with unresolved feedback/unknown execution kept
explicit; it must not stamp every associated conversation complete.

## Loop decisions — alternatives and recommendation for review

Jack Heart asked during kickoff whether `loop-decide` can be simplified or removed.
No option is accepted. Jack subsequently clarified that a replacement must exist
before removal and that exploration must not block the accepted work. **Required
baseline: retain loop-decide and existing templates**, receiving conversational
feedback instead of ready/complete receipts. The optional proposal below is a
separate review choice, not part of the deletion cut or its acceptance dependency.

Current main's `pursue.yaml` has an autonomous decider after
implement → compress → refresh, and another after interactive demo. `task-design`
has kickoff → review-design without a return edge. `engine/flow.rs` explicitly
rejects `human: true` with `repeat`; `FlowOutput::for_step` excludes human nodes.
Those are implementation rules to change, not product constraints.

| Decision owner | Autonomous implementation loop | Design/review iteration |
| --- | --- | --- |
| Task conversation decides everything | Every pass returns to the conversation, interrupting interactive work and coupling background progress to its availability. | Can interpret feedback, but must not substitute its preference for Jack's direction. |
| Jack chooses through ordinary feedback | Makes unattended implementation require Jack on every pass. | Direct and sufficient when the feedback answers the pending boundary; no second confirmation or interpreting Run. |
| Autonomous Flow decider chooses everything | Fits evidence-based autonomous iteration without touching the conversation. | Extra Run can reinterpret clear feedback, or mistake unresolved discussion for permission. |
| **Recommended split** | Keep one explicit autonomous decider where independent judgment is needed. | The ongoing conversation turns Jack's feedback into the exact review outcome and authored edge directly. |

The clearest optional direction is removing post-review `loop-decide` from authored templates, while retaining
the standalone skill for autonomous loops. This is smaller than changing `realign`
into an evaluator: its current skill explicitly supplies facts without selecting
navigation, and `refresh` ends at realign, not QA. Folding that decision into the
implementation agent would remove an independent judgment and change the process
without evidence. Full removal is possible only by giving another step those explicit
success/no-progress criteria; it is not achieved by hiding the same prompt elsewhere.
No extra generic evaluator, timer, numeric pass limit or permanent Task supervisor.

Optional template cut, only after Jack selects the replacement:

- `task-design`: give `review_kickoff` a backward edge to `kickoff`. Small wording
  changes stay in the ongoing review conversation; a needed fresh investigation
  returns through the declared headless kickoff edge. Advance proceeds to pursue.
- `pursue`: retain the autonomous `decide` after refresh with its existing edge to
  implement. Put `repeat: {from: implement}` on `review_delivery` and delete the
  following `decide_delivery` node. Review feedback can now return directly to
  implementation or advance to the caller's next stage.
- Keep captured old graphs unchanged as historical evidence. New template capture
  selects the revised edges; source editing never rewrites a live invocation.
  Preserve any publication/review gates and their order. This change supplies no
  new authorization to publish, land or do work outside the accepted scope.

For that optional replacement, extend the structured output contract to carry
`advance`, `iterate` or `stop`, with the exact feedback input-event references.
The conversation's agent interprets ordinary language within the selected review;
the runtime validates identity, successful turn completion and legal captured edge,
not semantic truth. “Looks good, continue” can advance; “Change the navigation and
show it again” can iterate; a question, unrelated message or ambiguous multi-Flow
feedback yields no decision and the runner keeps waiting. The agent answers or
clarifies naturally in the same conversation. A mere streamed mention of a decision
cannot settle anything; only the successful turn's exact validated result can.

“Stop this work” returns stop for the selected invocation, records that outcome and
ends its runner without completing the conversation. “Hold on, let's discuss” leaves
the pending review intact with no terminal result. “Continue” later can answer that
same boundary if its runner is still alive. After stop/crash, continuing requires a
fresh caller-chosen invocation from inspected evidence, not resume. A request to
change work with no legal backward edge remains discussion with that limitation
visible; it cannot invent an arbitrary cursor jump. Normal process interruption is
still available independently when a runner is executing rather than at review.

Autonomous deciders retain Advance/Iterate/Blocked from evidence; Blocked ends this
runner and returns its reason through ordinary output for caller recovery. A failed
or malformed output cannot imply advance. Keep existing same-Session output repair;
never rerun implementation just to repair decision JSON. Interactive ambiguous prose
is normal conversation, not malformed output or a failed Flow.

Optional replacement acceptance must include clear rejection, clear approval, an unresolved question,
“hold”, stop then later continuation, two reviews with an ambiguous “yes”, repeated
provider events, and autonomous progress/no-progress decisions. No Ready/Complete
command, button, secret phrase or extra acknowledgment may appear in those paths.
The boundary between actionable feedback and ongoing discussion is a product choice
for Jack to review; source-event correlation cannot prove an agent interpreted intent
correctly. Preserve the authored transcript and make the resulting next action visible.

## Delete — do not maintain

The architectural cut is indivisible; these are removal targets, not components to
modernize first. Paths below are under `rust/loopflow/src/` unless qualified.

- `durable.rs::{TaskWorkerOwner, TaskWorkerClaim, TASK_WORKER_CLAIM_ENV}` and the
  mutable controller portion of `FlowSession`/`FlowTurnSelection`; retain exact
  Session completion selection and ordinary process/Session fencing.
- `tasks.current_invocation_id`; `store/flows.rs` and `store/sqlite/flows.rs`
  managed selection, `start_task_flow`, `claim_task_worker`, `handoff_task_worker`,
  `reclaim_task_worker`, `checkpoint_flow`, `retry_flow`, `recover_flow`,
  `reserve_task_review`, `complete_task_review`. Replace mixed history writes at
  their owner, rather than deleting their retained data.
- `ops/run.rs::TaskWorkerExec`, `controller/task` worker/review progression,
  Task `__worker`, saved `flow resume`, `ops/flow_run.rs::exec_driver`, and managed
  branches in `lf/commands/flow.rs`. Keep shared engine graph/cursor traversal.
- New `FlowReview` Session creation and review-complete-to-driver restart in
  `ops/flow_session.rs`; `serve-flow` and managed review launch/token branches
  in `ops/human_session.rs`. Preserve existing review histories during conversion.
- `lf/mod.rs::SessionCommand::{Ready, Complete}`, their dispatch in
  `lf/commands/session.rs`, `ops/human_session` ready/complete operations,
  `SessionActionKind::Complete`, Ready lifecycle and live `ready_summary` reads.
  Cut `RegistryQuery.completeSession`, `SessionsView` completion/pane-removal
  behavior and associated Swift action/DTO fixtures together. Rewrite builtin
  `repo_operate`, `advance`, generated review/primary instructions, API docs and
  lifecycle tests around conversational results. Keep provider Completed events,
  historical completion timestamps and primary-successor bookkeeping for their
  separate meanings; do not bulk-delete anything named complete.
- Resume scheduling in `ops/task_automation.rs` and cron consumers; singular-Flow
  conditions/actions in `ops/task_execution.rs`, `ops/task_flow.rs`,
  `ops/task_actions.rs` and Task controller projections. Retain unrelated cron
  merge settlement, CI repair and release work.
- `FlowFilter.managed`, inventory managed flags, Task-work managed badges and
  Swift pinned-Flow mutable controls. Cut over `TaskStatus`, `TaskWork`, Session
  participation and Flow DTOs, `RegistryQuery`, Task history filtering and CLI
  status/roadmap together. iOS consumes shared models; inspect its compilation too.
- Exclusive claim/reclaim/restart tests and fixtures, including managed cases in
  `task_restart_tests`, `task_initialization_tests` where still present after sync,
  and the installation harness. Replace their promised behavior with ordinary
  Flow/caller-recovery tests, not renamed controller tests.

Landing is a concrete deletion hazard: `pr_landing.rs::landing_has_pending_flow`
and operation receipts can delay settlement for exact Flow work. Retain immutable
Flow/Exec/PR-operation linkage and delivery idempotency. Remove the assumption that
one Task-selected Flow supplies delivery authority. A merge receipt cannot resume a
runner or complete a review; unknown work remains unknown. Preserve existing Task
admission/completion policy while adapting its evidence readers; policy redesign
stays LOO-367. Update builtin Task-operation/session guidance and architecture/docs
so they no longer prescribe the deleted worker or retry APIs.

One draft migration, generated with `uv run python scripts/new_migration.py task_flow_observations`
after integration, converts the released schema directly. Preserve every invocation,
Session, event, exact review and effect receipt; archive last legacy position as
historical evidence, not resumable state. Existing pending reviews remain visible
as unresolved historical boundaries attached to their original conversations; they
are not auto-approved, discarded or silently reassigned. No live Home conversion
or active worker interruption is authorized by this design. Conversion requires
confirmed quiescence of writers in that Home; an isolated copy proves migration.
Do not maintain two executors to avoid a coordinated cutover. Review must approve
how unfinished legacy reviews are recovered before rollout.

## Alternatives and failure analysis

| Mechanism | Consequence |
| --- | --- |
| Live runner suspended at review — proposed | Keeps exact graph/loop continuity with only in-memory progression; costs one sleeping process per waiting Flow and loses continuation on crash. |
| Exit at every review and serialize successor | Reintroduces a durable continuation/controller under another name. Rejected. |
| Exit and let the conversation author a fresh Flow after every review | Simplest process lifecycle, but loses automatic authored return edges and fragments one Flow's graph/history. A product alternative requiring Jack's choice. |
| Keep database cursor but remove managed flag | Simplifies Task special cases but retains recovery/control architecture Jack wants removed. Rejected. |
| New review chat or second provider attached to the conversation | Breaks the accepted experience or duplicates the execution owner. Forbidden. |

Success means Jack can discuss two background efforts without tracking terminals,
then return days later to the same files and useful history. Failure would be a
hidden second controller, review feedback applied to the wrong loop, a native draft
lost to injected input, or delivery duplicated after an uncertain crash. Exact event
references, one conversation driver, authored review feedback and caller inspection
address these independently. Simulated review caught two traps: a saved successor
is a cursor, and `send_input` support is not native conversation integration.

The optional loop replacement has separate deletion targets: post-review
`decide_delivery`, the human-plus-repeat prohibition, and exclusive graph/output
fixtures. Its full template/FlowOutput/transition changes are described above.
They are not required for this runtime cut; no unconditional removal of the
loop-decide skill or its autonomous use is planned.

## Internal slices and remaining workspace scope

**This slice: kickoff only.** Source/read-contract review and draft design; no
production or migration changes. Interactive review-design precedes implementation.

After acceptance, one coherent runtime/UI change:

1. Integrate current main through the supported sync operation. Delete Task worker
   authority at its deepest store/driver boundary while converting invocation/event
   ownership and the minimum DTO/consumer set together. Prove ordinary mechanical
   and agent Flows, effect receipts and crash-without-resume on a disposable Home.
2. Extend existing Task primary selection and conversation input ownership. Prove
   two reviews and draft retention in one existing provider conversation before
   connecting all authored human steps. A native-delivery failure is not grounds
   to restore separate review Sessions. Remove the old review-launch paths.
3. Cut all Flow views over to the same lazy detail/graph renderer and implement
   Waiting through shared Rust projection and provider mappings. Keep pure selection
   local; no layout/attention refresh launches work. Finish removal of managed DTOs,
   old controls, automation and exclusive tests. No intermediate release with both
   execution paths is planned.
4. Complete configured workspace proof and repair defects it exposes, then retained
   Flow-default/source editing and website work below. Source inspection and
   simulated transports do not supply configured proof.

Retained work:

- Real provider/Flow continuation, owning-Home remote association, cross-Task
  focus/input and process retention, same-byte symlink/read-only draft transitions.
  Use a compatible private Home copy; never migrate the live Home as test setup.
- At least twenty comparable retained-layout actions against proposed p95 <100 ms,
  accepted input to usable/rendered content with identities, retained input and
  failures. Record idle CPU/process counts before/after with several Tasks open.
  Existing next-main-callback signpost is scheduling evidence only. Keep Product's
  hierarchy_interaction_ms and task_workspace_ready_ms separate; no speedup claim
  without comparable baselines. Retained layout actions make zero subprocess/network
  calls; selected Flow history loads lazily rather than decoding all captures.
- Expose Project's existing recommendation as Default Flow in Wave settings,
  preserving KRs/targets and explicit Task choices. No chapter reset or separate
  Task execution engine. Validate the actual checkout before capture.
- Edit Flow opens its real source through the existing editor. Builtin customization
  explicitly creates `.lf/flows/<canonical-name>.yaml`. Choose a source for composed
  edges, never serialize the display graph as authored YAML. Invalid YAML stays saved
  and visibly invalid, without builtin fallback. Refresh catalog after save. Running
  invocations stay pinned; saving affects future launches in that checkout only.
- Align website with delivered Sessions/Runs, files, background work and review
  behavior; remove obsolete resident/chat claims. No promotion before delivery.

Exclusions: new general messaging, automatic recovery service, new Run database
entity, new terminal grid, Project reset, Task completion-policy redesign,
permission automation, Claude hooks/SDK, cross-checkout Flow edit propagation,
and claiming external-product progress from Loopflow self-hosting.

## Done when and unattended acceptance

Gate owns the full affected checks once. Implement/compress use build plus the
focused case of their slice. All fixtures isolate Home/authority and fake external
side effects; no credentialed provider launches in unit tests. Preserve TESTING.md's
requirement to clear inherited LF/LOOPFLOW authority and pin the compiled fixture
CLI; schema materialization runs in disposable source, not this working checkout.
Commands below are the concrete target check plan; added cases are not present yet.

| Check command | Required observable result |
| --- | --- |
| `cargo test -p loopflow --test flow_tests` | Extend public CLI cases: ordinary foreground/detached Flows use the same engine; two associated Flows; exact nested review feedback; duplicate/stale feedback; discussion without a result stays pending; removed ready/complete commands are rejected; runner death before/after effect receipt; no automatic restart or new review Session. |
| `cargo test -p loopflow --test session_lifecycle_tests` | Same Session/provider identity through review and mode changes; Task primary remains a member; repo/Wave exclusions, explicit bindings and remote owning-Home identity retained. |
| `cargo test -p loopflow --lib ops::human_session::tests` | Waiting at 120 seconds using an injected clock; immediate yield/pending input, new activity, closed/failed/unknown states; filtering before pagination. |
| `cargo test -p loopflow --lib harness::` | Recorded provider traces prove tool correlation, duplicate/out-of-order events, reconnect uncertainty and new-activity clearing; Claude binary stream-json only. |
| `cargo test -p loopflow --lib engine::flow_graph::tests` | Every Flow shares exact captured traversal, nested return edges and occurrence identity, including feedback to the following decider and autonomous outcomes; direct review edges only if separately selected. |
| `cargo test -p loopflow --test dto_fixtures` | Session, Task and Flow fixture shapes agree with shared Swift consumers; no compatibility defaults or managed fields. |
| `cargo test -p loopflow --test pr_tests` | Exact external-effect and landing receipts survive controller deletion; no duplicate publication/landing on observation or feedback event replay. |
| `uv run python scripts/materialize_rust_tests.py -- cargo test -p loopflow --lib store::` | Populated released frontier converts directly to the final draft, retaining captures, Sessions, pending legacy review evidence and effect history without resumable controller state. Run from disposable source. |
| `cargo build -p loopflow --bin lf` then `scripts/test_desktop.sh --no-parallel -Xswiftc -gnone` | Real CLI transport plus headless view/model cases cover equal Flow detail, Waiting, filtered inventory retention, Task primary selection, conversational feedback without completion controls, files and layouts. Extend TaskFlowProofTests, WorkspaceNavigationTests, SessionsStoreTests and DTOFixtureTests. |
| `uv run python scripts/test.py --loopflow` | Both supported Mac build paths and configured headless checks compile. Unavailable platform checks go to capable CI; no display/Automation prerequisite. |
| `uv run python scripts/check_architecture.py` and `cargo clippy --all-targets -- -D warnings` and `cargo fmt --check` | Ownership map, docs and static checks contain one implementation. |

Cross-layer gate scenario: a real compiled CLI in a disposable Home creates one
Task conversation and two Flows with simulated provider transport. Both stop at
different captured reviews. Shared list/detail JSON is decoded by the production
Swift readers; the headless view model selects one review and submits ordinary conversational
feedback through the existing input surface. The simulated provider returns its
correlated review result through the real Session event recorder. Exactly one Flow consumes its exact Session feedback event;
its authored successor/decider runs, the other waits, and the original conversation ID and
file/layout state remain. Kill the first runner, reread through both consumers and
prove retained history plus no child restart. Extend existing flow and Swift
transport fixtures; no test-only production factory or independent UI state machine.

Configured demo remains separate: one actual supported provider through launch,
review delivery, multiple discussion turns, conversational feedback and continued
background work in the same Task conversation, then remote/Home and retained-draft
scenarios above. Native
surface judgment and compositor measurements belong to the configured demo, not
headless test substitutes. Other providers require their own continuity evidence.

The supplied chapter has no metric targets. Its KRs require Jack's confirmation of
three consecutive working days mostly in Desktop and three real sessions of at
least two hours without crash/lost access forcing a move. Record dates/durations and
Jack's verdict after use; local tests or this design cannot mark those KRs complete.
The design serves Product's workspace contract but earns no Cube/Etude/Kata/Hootro
weekly-progress credit.

Checks: `git diff --check` passed; read-only source/contract review completed against the two commits above; production tests/builds not run (prose-only kickoff); configured handoff/performance proof remains with implementation/gate/demo.
