# Task conversation and ordinary background Flows

October 4, 2026. LOO-353, Product. **Design review finished and approved by
Jack Heart.** Jack requested a further kickoff pass, then explicitly requested
marking this review finished and approved. Approval covers the decisions in this
conversation; the implementation plan below is being reconciled to the final cut.

Final approved direction supersedes the earlier single-graph/segment proposal:
a higher-level human workflow has conversation-stage nodes and edges labeled with
operational Flows. The ongoing native Task conversation owns navigation without a
shared playhead or review-settlement API. Operational Flows contain autonomous
loops and XORs only; they cannot contain human workflows. Zero-human defaults name
ordinary operational work. Autonomous loops end with a decider, named loop-or-next
by default. Internal IDs never require duplicate authored id/name fields.

## Outcome and demo

Jack keeps designing, discussing and reviewing in one ongoing Task conversation.
Headless work appears separately as Runs. Returning to a Task preserves the same
conversation, files, drafts, shells and layout; understanding background work does
not require finding a privileged Flow or opening a new review conversation.

Demo: in a native Task conversation, launch two ordinary background Flow segments.
Each exits before its next human step and returns its exact review position and
results. The same conversation handles one review and launches its next segment
with Jack's feedback; the other stays pending. Continue through nested loops with
repeated skill names without confusing occurrences. Terminal choice does not change
this contract. A mid-segment crash retains evidence but never restarts automatically.
This is proposed behavior, not a claim of a working demo.

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

## Chosen architecture — revised during review

### Caller-launched segments and exact positions

On October 4, Jack Heart clarified that Loopflow launches native provider terminals;
Task Sessions also run in a preferred external terminal. Jack requested an API to
run a Flow until its first human step, and to start at a human or other step. Jack
then required resilience to deeply nested loops and repeated uses of a skill.
These decisions replace the sleeping-runner and automatic native-input proposals.
The detailed API below remains a proposal; implementation is not yet approved.

One captured FlowSession retains the resolved graph and history across explicit
segment launches. Each segment is an ordinary lf process with its own Exec; no
Task worker, privileged managed Flow, supervisor or automatic restart exists.
The live process traverses the captured graph until just before a human step,
returns a boundary result and exits. The native conversation agent reads that
result, follows the captured review instructions with Jack, and explicitly invokes
the next segment with the feedback. Neither Desktop nor a harness owns the native
composer. No injected turn, second provider or new review conversation is needed.
Ordinary background output inspection supplies results to a caller that detached.

Proposed API semantics (command spelling remains open):

- Run from the beginning or an exact selected position, stopping before the next
  human step. Return done, failed, or a human boundary with preceding result
  references, captured review instructions and an opaque position reference.
- Starting **at** a human step returns that boundary without executing it. Starting
  **after** that boundary supplies the conversational feedback and follows the
  captured successor, following its selected authored edge. No Session-completion API.
- Starting at another step explicitly selects an entry point; it does not assert
  skipped work succeeded. A new entry must establish its enclosing branch and loop
  context. Reusing an existing boundary preserves that exact context.
- A skill label is display text, never an execution address. Resolve an authored
  node ID or a captured structural address; ambiguous shorthand shows the matching
  occurrences rather than choosing the first. Returned opaque references require
  no manual reconstruction by the agent or Jack.

### Minimal authored config; internal execution identity

Jack Heart suggested an ID/name split while rejecting mandatory duplicated fields
such as `id: mywave` and `name: mywave`. Keep the existing minimal Flow syntax:
skill references and composition express the work. Capture assigns internal node
keys automatically; execution addresses never require authors to write those keys.
Repeated uses of the same skill remain legal without added labels.

Proposal: allow one optional authored name only when it helps a person reference
or read a particular occurrence. Do not require an authored ID alongside it.
The exact YAML spelling needs the broader config-minimality pass; this design does
not add a second naming field. Display falls back to the skill/operation label.
Name lookup resolves to captured identity and reports ambiguity with structural
context. The returned continuation reference already identifies the exact occurrence,
so ordinary continuation needs no name lookup or new config annotation.
Generated identity is stable within the immutable capture, not promised across
source edits or independent captures. A human-readable name is not a loop counter
or a unique execution identity.

An exact position carries the captured invocation/graph identity, unique node key,
full enclosing execution path and loop/return state, and boundary event identity.
Every nested frame identifies its structural node/edge and its iteration, outermost
first; branch choices and each backward edge's counters are retained. Depth is a
sequence, not a fixed number of fields. The same node in a later outer iteration is
therefore distinct even when an inner counter has reset. Attempts/segment Execs
remain separate from logical occurrence identity.

Reuse the engine's captured traversal and graph keys rather than a second path
parser in Desktop. `engine/flow_graph.rs` already distinguishes captured preorder
keys from labels and includes XOR alternatives. Verify the existing execution
cursor contains all state needed to reconstruct the exact successor; graph key
plus a scalar iteration is insufficient. Composed copies of the same Flow and
repeated skills must retain separate keys. Source edits never reinterpret an old
reference. Pretty labels may show the path and iterations, but callers pass the
opaque reference back unchanged.

This explicitly retains a serialized continuation at clean human boundaries.
The earlier blanket rejection of saved positions is superseded by Jack's segment
API direction. It does not retain the Task-worker controller: nothing observes
that position and launches work automatically, and it cannot mutate a running
segment. No special mutable Task pointer or worker claim/lease is retained.

### Loop configuration under review

Jack Heart prefers an explicit loop node with a `step` naming its evaluator, and
initially selected a pointer to its destination: a direct graph representation.
Jack is also open to declaring the loop at the top. Jack confirmed that the decider
executes last, after the repeated work, whether autonomous or human. Top declaration
versus backward-pointer syntax is not yet settled; neither changes execution order.

Pointer-form proposal:

```yaml
- implement
- compress
- loop: implement
  step: loop-or-next
- loop: implement
  step: demo
  human: true
```

Here each deciding node has a backward target and a forward successor. A plain
skill reference can resolve a unique target; optional authored names disambiguate
repeated skills. The two decisions may share a backward target. Preserve this graph
expressiveness rather than assuming every loop region nests without overlap.

Top-declaration alternative shown during review:

```yaml
- loop:
    - implement
    - compress
  step: loop-or-next
- step: demo
  human: true
```

The enclosing `step` runs after that loop's body. This example has no demo return
edge and is not equivalent to the two-pointer example. Nested regions are easier
to scan, but shared/overlapping return targets need an explicit representation.
Resolve the authoring choice before changing parsing; keep one canonical form
unless both forms have a demonstrated purpose. Earlier `repeat` syntax examples
below describe existing engine/template changes, not the selected new YAML spelling.

### History, feedback and recovery

Retain SQLite invocation and event history, exact Session/Exec references, graph
captures, attribution and external-effect receipts. Each clean boundary records
its immutable continuation and preceding results before the process exits.
Session history owns the conversation; Flow history references feedback evidence
without duplicating the transcript. The precise native feedback extraction and
submission format remains to be designed; do not assume headless structured-output
capture exists in a native terminal conversation.

Questions, ambiguous feedback and “hold” remain discussion; the agent launches no
next segment. Clear actionable feedback can supply the boundary result. The conversation chooses the authored review edge from that feedback. Autonomous
loops retain a separate decision skill, named loop-or-next below. A position reference identifies
work, not Jack's authorization to publish or land.

Proposal: explicit next-segment admission records which boundary result and input
it consumed, using ordinary execution exclusion and an idempotent receipt. Repeating
the identical submission reports that execution; competing or changed feedback
cannot silently launch a duplicate. This is an effect-safety requirement, not a
Task worker claim or automatic recovery mechanism. Its concrete transaction must
be reviewed alongside existing delivery receipts. Deliberate reruns create visibly
new execution evidence rather than altering consumed history.

A clean human boundary can be continued later without a sleeping process. A crash
mid-segment is different: retained effects may be uncertain, and the last boundary
is not permission to replay everything after it. The caller inspects results and
effect receipts, then explicitly selects fresh work or an exact entry point.
Unknown remote liveness stays unknown. Reading history, reopening the conversation,
closing a pane and receiving a merge receipt never restart execution.

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

## Loop decisions — separate autonomous and conversational decisions

During October 4 review, Jack Heart confirmed two distinct mechanisms: the Task
conversation interprets review feedback; autonomous work retains a decision skill.
Jack selected `loop-or-next` as the replacement name for `loop-decide`. The rename and
selected split are design direction, not permission to begin implementation.
Earlier proposals to keep a post-review decider as the required baseline are
superseded. Exact template syntax and feedback submission remain proposals below.

Jack also clarified that control structure and decision owner are independent:

| Structure | Autonomous decision | Human decision |
| --- | --- | --- |
| XOR | An agent selects an authored alternative. | The Task Session interprets Jack's conversational choice and executes that alternative. |
| Loop | loop-or-next chooses return or forward progression. | The Task Session interprets Jack's direction and executes the authored return or forward edge. |

The Flow annotates human decision boundaries, including XORs and loops. Run-until-
human stops before any such boundary, not only a skill marked as interactive.
The returned position includes the captured alternatives or return edge. Autonomous
XORs continue to use their branch-selection mechanism; loop-or-next names the
binary loop decision, not a universal router. Exact annotation syntax remains draft
and must preserve minimal authored config without mandatory ID/name duplication.

The Task Session “softly listens” to ordinary conversation: clear direction supplies
an exact choice and launches the next segment; a question stays discussion; unclear
or competing references prompt clarification. No secret phrase, fixed approval form,
extra confirmation or background listener process. The native conversation agent
executes the decision through ordinary lf commands. A Flow defines available paths;
it does not authorize the agent to infer approval from silence or unrelated text.

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
| **Selected split** | Keep one explicit autonomous decider where independent judgment is needed. | The ongoing conversation turns Jack's feedback into the exact review outcome and authored edge directly. |

Remove post-review `loop-decide` from new authored templates; retain the autonomous
skill under the chosen name `loop-or-next`. This is smaller than changing `realign`
into an evaluator: its current skill explicitly supplies facts without selecting
navigation, and `refresh` ends at realign, not QA. Folding that decision into the
implementation agent would remove an independent judgment and change the process
without evidence. Full removal is possible only by giving another step those explicit
success/no-progress criteria; it is not achieved by hiding the same prompt elsewhere.
No extra generic evaluator, timer, numeric pass limit or permanent Task supervisor.

Proposed template changes for the selected split:

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

For conversational review, the explicit next-segment launch carries
`advance`, `iterate` or `stop`, with the exact feedback input-event references.
The conversation's agent interprets ordinary language within the selected review;
the runtime validates the exact boundary and legal captured edge, not semantic
truth. Native feedback evidence must be supplied through the segment API; do not
assume the native conversation has a harness structured-output contract. “Looks good, continue” can advance; “Change the navigation and
show it again” can iterate; a question, unrelated message or ambiguous multi-Flow
feedback yields no decision and no next segment launches. The agent answers or
clarifies naturally in the same conversation. A mere streamed mention of a decision
cannot launch work; the explicit segment API supplies the exact decision and feedback.

“Stop this work” returns stop for the selected invocation, records that outcome and
ends further progression without completing the conversation. “Hold on, let's discuss” leaves
the pending review intact with no terminal result. “Continue” later can explicitly launch from that same clean boundary. After a
mid-segment crash, the caller inspects effects before choosing another entry point. A request to
change work with no legal backward edge remains discussion with that limitation
visible; it cannot invent an arbitrary cursor jump. Normal process interruption is
still available independently when a runner is executing rather than at review.

Autonomous deciders retain Advance/Iterate/Blocked from evidence; Blocked ends this
runner and returns its reason through ordinary output for caller recovery. A failed
or malformed output cannot imply advance. Keep existing same-Session output repair;
never rerun implementation just to repair decision JSON. Interactive ambiguous prose
is normal conversation, not malformed output or a failed Flow.

Acceptance for the selected split must include clear rejection, clear approval, an unresolved question,
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
| Explicit segments ending before human steps — selected direction | Preserves native conversations and exact captured continuation without a sleeping process or automatic recovery. |
| Live runner suspended at review | Superseded by Jack's segment API direction. |
| Fresh authored Flow after every review | Needlessly discards captured nested-loop continuity. |
| Keep Task worker under a new name | Violates the complete deletion requirement. |
| New review chat or replacement composer | Violates the native Task conversation contract. |

Review must catch addressing by skill label, losing outer-loop state, silently
replaying effects after a crash, and interpreting a saved position as authorization.
Clean boundary continuation and interrupted execution require different recovery.

The selected split removes post-review `decide_delivery` and the human-plus-repeat
prohibition, adapting exclusive graph/output fixtures. Rename the autonomous skill
and its catalog, templates, docs and tests together; do
not leave a duplicate skill or alias. Preserve historical captured graphs and step
evidence. The live catalog rename must not make an old captured segment impossible
to continue; captured skill content/provenance must remain resolvable independently
of the current catalog name.

## Internal slices and remaining workspace scope

**Status: interactive design review.** Jack selected caller-launched segments and
exact positions. API spelling, feedback representation and admission semantics
remain draft; no production or migration changes are approved by this document.

After acceptance, one coherent runtime/UI change:

1. Integrate current main through the supported sync operation. Delete Task worker
   authority at its deepest store/driver boundary while converting invocation/event
   ownership and the minimum DTO/consumer set together. Prove ordinary mechanical
   and agent Flows, effect receipts and crash-without-resume on a disposable Home.
2. Extend Task primary selection and implement exact segment boundaries and explicit
   continuation. Prove two reviews in one native provider conversation, including
   an external terminal, without changing its composer or losing drafts. Remove
   separate review-launch paths; prove deep nesting and repeated skill addressing.
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
| `cargo test -p loopflow --test flow_tests` | Extend public CLI cases: ordinary foreground/detached Flows use the same engine; two associated Flows; clean segment exit and explicit exact continuation; deep nested/repeated-skill addressing; duplicate/stale feedback; discussion without a next launch stays pending; removed ready/complete commands are rejected; runner death before/after effect receipt; no automatic restart or new review Session. |
| `cargo test -p loopflow --test session_lifecycle_tests` | Same Session/provider identity through review and mode changes; Task primary remains a member; repo/Wave exclusions, explicit bindings and remote owning-Home identity retained. |
| `cargo test -p loopflow --lib ops::human_session::tests` | Waiting at 120 seconds using an injected clock; immediate yield/pending input, new activity, closed/failed/unknown states; filtering before pagination. |
| `cargo test -p loopflow --lib harness::` | Recorded provider traces prove tool correlation, duplicate/out-of-order events, reconnect uncertainty and new-activity clearing; Claude binary stream-json only. |
| `cargo test -p loopflow --lib engine::flow_graph::tests` | Every Flow shares exact captured traversal, nested return edges and occurrence identity, including human and autonomous XORs and loops, direct conversational review edges and autonomous loop-or-next outcomes. |
| `cargo test -p loopflow --test dto_fixtures` | Session, Task and Flow fixture shapes agree with shared Swift consumers; no compatibility defaults or managed fields. |
| `cargo test -p loopflow --test pr_tests` | Exact external-effect and landing receipts survive controller deletion; no duplicate publication/landing on observation or feedback event replay. |
| `uv run python scripts/materialize_rust_tests.py -- cargo test -p loopflow --lib store::` | Populated released frontier converts directly to the final draft, retaining captures, Sessions, pending legacy review evidence and effect history without resumable controller state. Run from disposable source. |
| `cargo build -p loopflow --bin lf` then `scripts/test_desktop.sh --no-parallel -Xswiftc -gnone` | Real CLI transport plus headless view/model cases cover equal Flow detail, Waiting, filtered inventory retention, Task primary selection, conversational feedback without completion controls, files and layouts. Extend TaskFlowProofTests, WorkspaceNavigationTests, SessionsStoreTests and DTOFixtureTests. |
| `uv run python scripts/test.py --loopflow` | Both supported Mac build paths and configured headless checks compile. Unavailable platform checks go to capable CI; no display/Automation prerequisite. |
| `uv run python scripts/check_architecture.py` and `cargo clippy --all-targets -- -D warnings` and `cargo fmt --check` | Ownership map, docs and static checks contain one implementation. |

Cross-layer gate scenario: the compiled CLI in a disposable Home runs two captured
Flows with simulated headless providers to different human boundaries. Both segment
processes exit. Production Swift readers display their exact pending positions and
history. An explicit CLI continuation with fixture feedback advances only one Flow;
the other stays pending. Repeated submission does not launch duplicate effects.
Continue through at least three nested loops, repeated skill names, repeated composed
Flows and both human/autonomous XOR and loop decisions; compare the full visit sequence to uninterrupted engine
traversal. Include inner-counter reset on outer return and edits to source YAML after
capture. Arbitrary entry exposes skipped prerequisites rather than fabricating success.
Kill a segment before/after an effect receipt and prove no automatic child restart.

Configured demo remains separate: an actual native provider conversation in Desktop
and in an external terminal launches work, reads its boundary, discusses the review
across multiple turns and explicitly continues it. Preserve Session/provider identity,
composer drafts, files and layout. No custom composer or injected review turn.
Remote/Home, retained-draft and compositor proof remain required; simulated native
feedback is not configured provider proof. Other providers need continuity evidence.

The supplied chapter has no metric targets. Its KRs require Jack's confirmation of
three consecutive working days mostly in Desktop and three real sessions of at
least two hours without crash/lost access forcing a move. Record dates/durations and
Jack's verdict after use; local tests or this design cannot mark those KRs complete.
The design serves Product's workspace contract but earns no Cube/Etude/Kata/Hootro
weekly-progress credit.

Checks: `git diff --check` passed; read-only source/contract review completed against the two commits above; production tests/builds not run (prose-only kickoff); configured handoff/performance proof remains with implementation/gate/demo.
