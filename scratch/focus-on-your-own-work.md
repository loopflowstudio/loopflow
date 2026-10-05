# Task conversation and ordinary background Flows

October 4, 2026. LOO-353, Product. **Design review finished and approved by
Jack Heart.** Jack requested further kickoff reconciliation and continuation
through the existing saved pursue Flow. Reconciliation is complete; implementation
and configured acceptance remain outstanding.

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

Demo: in the same native Task conversation, launch two ordinary background
operational Flows. Inspect each invocation's live graph and results without losing
files, drafts or layout. Discuss one result with Jack and choose its next workflow
edge, launching fresh operational work; leave the other discussion unresolved.
Repeat an autonomous loop with nested XORs and repeated skill names, preserving
exact occurrence history. The same path works in an external terminal. A crash
retains evidence and never restarts automatically. This demo remains unexecuted.

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
  workflow choice without completing the conversation or settling a Flow review. No renamed
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
`6513477a5`; the superseded worker-start failure and repair proposal remain at
`5090f672e:scratch/task-workspace-continuation.md`, outside active scratch.
That audit performed no sync, worker start, provider launch or live migration.

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
| `ops/flow_session.rs` | `reserve` creates a new `FlowReview`; `complete` closes it and launches a resumed driver. Delete review settlement; ordinary conversation launches the next operational Flow. |
| `ops/human_session/primary.rs` | Scope admission, retained native identity, launch lock. Extend selection to Tasks without excluding Task primaries from membership. |
| `store/sqlite/flow_inventory.rs` | Indexed inventory and lazy detail already exist; managed bit/filter derives from the Task pointer. Reuse inventory/detail, remove privilege. |
| `TaskWorkView.swift`, `TaskFlowView.swift` | All-work rows versus richer selected-Flow graph. One detail view per selected invocation; no selected invocation acquires execution authority. |
| `harness/claude.rs` | Persistent binary stream-json stdin and retained provider ID already support successive inputs. This capability is not native-composer delivery and is unnecessary for workflow navigation. |

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

## Chosen architecture — reconciled October 4

### Human workflows and operational Flows

A human workflow is authored guidance: conversation-stage nodes joined by edges
that name operational Flows. The ongoing native Task conversation follows this
map, interprets Jack's direction and invokes an edge with ordinary `lf -b`.
No database row, shared cursor, pending-boundary token or review-settlement API
owns the conversation's position. Reopening a Task reads its conversation and
actual work history; it does not resume a workflow controller.

Operational Flows contain only autonomous work, loops and XORs. They may compose
other operational Flows; they cannot contain human workflows or human steps,
including through nested references and XOR alternatives. Validate the resolved
composition before launch. Human review skills remain usable in the ongoing
conversation; they are not executable human nodes in an operational capture.

Each edge launch creates an ordinary captured FlowSession with associated Execs
and AgentSessions. It ends completed, failed or blocked, with exact results and
effect receipts. Completion makes evidence available for discussion; it does not
move a human-workflow playhead. The conversation chooses another edge only when
Jack's direction and existing authorization support it. A question, hold, unrelated
message or ambiguous “yes” across two results launches nothing. Stop interrupts
an exact owned process if necessary; later continuation is a fresh explicit launch
after inspection, not a review completion or automatic replay.

Implementation choice: represent workflow definitions beside authored Flows in
`.lf/workflows/<name>.yaml`, using conversation-stage keys and edges with `from`,
`to`, and `flow`. Stage keys are the single authored name; optional review-skill
references supply instructions. This is a source format, not a new Work kind or
runtime store. Reuse catalog/source resolution and graph display primitives where
they apply, but never convert a workflow into the executable Flow graph. Show the
workflow map as guidance and actual Flow invocations as observed work; do not draw
a fabricated current-stage marker. Missing/invalid references stay visibly invalid.

For example, design discussion → delivery review can name `pursue`; delivery
review → delivery review can name `pursue` again for requested changes. A separate
accepted delivery edge can name the existing delivery operation. These are distinct
launches, with historical invocation identity, not mutations of one saved Flow.
Any publish/merge effect still requires the applicable authorization.

### Autonomous loops and exact history

Use one canonical loop form: an explicit deciding node with a backward target,
`loop: <target>`, and optional `step`, defaulting to `loop-or-next`. The decider runs
last, after the work. Ordinary unique skill names resolve a target; one optional
occurrence name disambiguates repeated labels, without mandatory id/name pairs.
Capture allocates internal graph keys. Resolve targets once at capture and report
ambiguous matches with structural context. Preserve shared/overlapping return
edges instead of restricting all loops to nested regions. Top-declared syntax is
not needed for this cut.

```yaml
- implement
- compress
- flow: refresh
- loop: implement
  step: loop-or-next
```

Keep the existing engine traversal and XOR selection, adapting its authoring and
captured representation. Each event identifies the captured graph, node key,
complete branch/loop path and per-return-edge counters, plus exact Session/Exec
references. Nested frames are a sequence; an inner counter resetting on an outer
return cannot collapse distinct occurrences. Repeated composed Flows keep separate
keys. Source edits do not reinterpret captured history. The live cursor is owned
by the executing process; persisted observations are evidence, never a restart
instruction or mutable Task pointer.

`loop-or-next` selects Advance, Iterate or Blocked from autonomous evidence.
Malformed output cannot imply advance; retain same-Session output repair instead
of rerunning implementation to repair decision JSON. Rename catalog, skills,
new templates, docs and tests together, without a duplicate alias. Historical
captures retain their original labels and evidence.

### History, launch and recovery

Retain SQLite invocation/event history, graph captures, attribution, exact
Session/Exec links and external-effect receipts. Ordinary launch admission and
process fencing prevent duplicate execution of the same admitted launch; Task
membership grants no privileged execution authority. Do not introduce feedback
submission receipts or idempotent review-consumption transactions: neither has a
boundary to settle in this architecture. Preserve existing operation idempotency
for publication and landing independently of workflow navigation.

A crash or uncertain remote liveness requires caller inspection of the existing
execution and effects before a fresh launch. Missing evidence stays unknown.
There is no automatic resume, sleeping runner, segment continuation token,
start-after-human API or background composer injection. The earlier segmented
single-graph proposal is preserved at `5090f672e:scratch/focus-on-your-own-work.md`
as superseded review history, not an implementation dependency.

### Current source and launch bridge

Read-only inspection on October 4 found local main at `a1d2f8a591e14f52d29b87fe56a27eccd5e31f8f`.
Its builtin `pursue` now ends after autonomous iteration and `pr-publish`; it no
longer contains demo or a post-demo decider. The Task's saved capture
`fbd24356-3d6f-415d-a236-16b4f6140c8f` still contains implement → compress → refresh
→ loop-decide → pr-publish → demo → loop-decide, at node 0 with no completed nodes,
no pending Session and no recorded failure. Installed `lf flow start` explicitly
continues saved progress; a template argument only supplies a new Task's Flow.

Jack requested continuing that exact saved capture while building its replacement.
That is an execution bridge, not permission to rewrite the capture or retain the
worker in the final product. Continue with `lf --task LOO-353 flow start`; do not
supply a replacement template, reset progress or start a parallel implementation.
If the retained runtime reaches a human review, report it to the calling
conversation without opening another interactive review Session. A command failure
is a blocker to report, not permission to edit the live Home or retry uncertain work.

### Primary selection and Waiting

Extend existing primary selection with `PrimaryScope::Task(TaskId)`. Ensure under
the existing scope lock; reuse an explicitly chosen existing Task conversation or,
on initial selection, the sole unfinished interactive Task conversation. If several
exist and none is selected, default to the most recently used interactive conversation
and preserve the rest. This is a reversible implementation default, not evidence of Jack's preference. If none exists, explicit Task conversation opening creates one
in the recorded checkout; read-only inventory never does. A Task primary is still a
Task member: narrow current primary exclusions to Repository/Wave. No Task Ctrl-C
reset behavior is introduced; ordinary interruption never restarts its Flow.

**Implementation default for Waiting: two minutes**, a single internal default, no settings UI.
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
Historical review evidence remains inspectable without becoming live control state.

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

## Template and consumer cutover

Separate human workflow definitions from operational templates. Move kickoff/review
and demo/revision guidance into conversation stages and operational edges. Keep
zero-human defaults named for ordinary work; Project's Default Flow selects an
operational Flow. Remove post-review deciders and all `human: true` execution paths
from new operational definitions. Autonomous XORs keep their existing branch
selection; conversational choices select workflow edges without structured review
output or a settlement command.

The same Task can show every associated Flow with equally rich lazy graph, progress
and output views. Swift consumes Rust's captured operational graph and actual
history, while source workflow guidance remains distinct. No selected row, primary
conversation or displayed workflow stage acquires process authority.

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
Do not maintain two executors to avoid a coordinated cutover. Keep legacy reviews
as unresolved history with their original conversations and result references;
caller inspection may choose fresh work but cannot settle or silently replay them.
Live rollout remains separate from this authorized implementation.

## Alternatives and failure analysis

The approved two-level model removes the need for human-boundary serialization,
feedback submission and shared navigation. Sleeping runners, segmented single
captures and renamed Task controllers recreate the deleted authority. A second
review conversation or composer breaks native continuity. Retain exact autonomous
occurrence history without confusing that evidence with a human-workflow position.

Review finding: the earlier plan's clean-boundary receipt and feedback validation
would have recreated the review handshake despite deleting Ready/Complete. Both
are removed from this plan. A second finding is that main's new pursue source does
not describe the Task's older saved capture; continuation must use the saved one.

## Internal slices and remaining workspace scope

**Status: design review approved; kickoff reconciliation complete.** The source
format, 120-second timeout and deterministic primary selection above are reversible
implementation choices. No missing product judgment blocks implementation. Live
migration, publication and configured usability verdicts retain their own boundaries.

One coherent runtime/UI change, with no intermediate release of two executors:

1. **This slice:** integrate current main through supported sync; remove Task-worker
   authority at store/driver ownership while preserving invocation/event/effect
   history and cutting over the minimum DTO consumers. Build the changed Rust code
   and run the focused ordinary-Flow crash/no-restart case in `flow_tests`.
2. Separate workflow source guidance from executable operational graphs; implement
   canonical autonomous loops/XOR composition and the loop-or-next catalog rename.
   Delete human-review launch/settlement APIs and their consumers together. Add
   ordinary `-b` launch, Task primary selection and caller-owned recovery paths.
3. Cut all Flow views to the shared lazy detail renderer; implement Waiting through
   Rust/provider mappings. Remove managed fields, controls, automation, exclusive
   tests and stale instructions. Verify the deletion inventory by final source
   search, distinguishing historical migrations/captures from executable paths.
4. Complete configured workspace proof and repair exposed defects, then retained
   default/source editing and website scope below. Native conversation navigation
   in Desktop and external terminals needs real configured proof, not input injection.

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
| `cargo test -p loopflow --test flow_tests` | Extend public CLI cases: ordinary foreground/detached Flows use the same engine; two associated Flows; autonomous completion; nested/repeated-skill history; reject human/workflow references in operational composition; ordinary admission and effect deduplication; removed ready/complete commands are rejected; runner death before/after effect receipt; no automatic restart or new review Session. |
| `cargo test -p loopflow --test session_lifecycle_tests` | Same Session/provider identity through review and mode changes; Task primary remains a member; repo/Wave exclusions, explicit bindings and remote owning-Home identity retained. |
| `cargo test -p loopflow --lib ops::human_session::tests` | Waiting at 120 seconds using an injected clock; immediate yield/pending input, new activity, closed/failed/unknown states; filtering before pagination. |
| `cargo test -p loopflow --lib harness::` | Recorded provider traces prove tool correlation, duplicate/out-of-order events, reconnect uncertainty and new-activity clearing; Claude binary stream-json only. |
| `cargo test -p loopflow --lib engine::flow_graph::tests` | Every Flow shares exact captured traversal, nested return edges and occurrence identity, including autonomous XORs, overlapping return edges and loop-or-next outcomes; workflow guidance never becomes an execution cursor. |
| `cargo test -p loopflow --test dto_fixtures` | Session, Task and Flow fixture shapes agree with shared Swift consumers; no compatibility defaults or managed fields. |
| `cargo test -p loopflow --test pr_tests` | Exact external-effect and landing receipts survive controller deletion; no duplicate publication/landing on observation or feedback event replay. |
| `uv run python scripts/materialize_rust_tests.py -- cargo test -p loopflow --lib store::` | Populated released frontier converts directly to the final draft, retaining captures, Sessions, pending legacy review evidence and effect history without resumable controller state. Run from disposable source. |
| `cargo build -p loopflow --bin lf` then `scripts/test_desktop.sh --no-parallel -Xswiftc -gnone` | Real CLI transport plus headless view/model cases cover equal Flow detail, Waiting, filtered inventory retention, Task primary selection, conversational feedback without completion controls, files and layouts. Extend TaskFlowProofTests, WorkspaceNavigationTests, SessionsStoreTests and DTOFixtureTests. |
| `uv run python scripts/test.py --loopflow` | Both supported Mac build paths and configured headless checks compile. Unavailable platform checks go to capable CI; no display/Automation prerequisite. |
| `uv run python scripts/check_architecture.py` and `cargo clippy --all-targets -- -D warnings` and `cargo fmt --check` | Ownership map, docs and static checks contain one implementation. |

Cross-layer gate scenario: the compiled CLI in a disposable Home launches two
ordinary operational Flows with simulated headless providers. Production Swift
readers show both graphs, exact histories and outcomes without managed privilege.
A conversation fixture reads the results and explicitly launches one next workflow
edge as a new invocation. No navigation/playhead row or feedback-settlement call is
written. Repeat observation alone launches nothing. Include three nested loop
contexts, shared return targets, repeated skill/composed-Flow occurrences and XORs;
compare the full visit sequence with engine traversal, including inner-counter
reset. Source edits leave prior captures unchanged. Kill an invocation before/after
an effect receipt and prove no automatic restart or duplicate publication/landing.
Reject human steps and workflow nesting even when hidden in composed XOR branches.

Configured demo remains separate: an actual native provider conversation in Desktop
and an external terminal launches work, reads its outcome, discusses it across
multiple turns and explicitly chooses another operational edge. Preserve Session/
provider identity, composer drafts, files and layout. Exercise clear rejection,
approval, unresolved questions, hold, stop/later continuation and ambiguous feedback
across two results; discussion alone never launches work. No injected turn, custom
composer, review token or Ready/Complete command. Remote/Home, retained-draft and
compositor proof remain required; fixtures do not prove configured intent handling.
Other providers need continuity evidence. Gate's headless checks run without a
display; missing configured demonstration remains with demo, never a fabricated pass.

The supplied chapter has no metric targets. Its KRs require Jack's confirmation of
three consecutive working days mostly in Desktop and three real sessions of at
least two hours without crash/lost access forcing a move. Record dates/durations and
Jack's verdict after use; local tests or this design cannot mark those KRs complete.
The design serves Product's workspace contract but earns no Cube/Etude/Kata/Hootro
weekly-progress credit.

Checks: `git diff --check` passed for prose reconciliation; saved Flow and local-main source inspected; no production build/test or configured demo run during kickoff reconciliation.
