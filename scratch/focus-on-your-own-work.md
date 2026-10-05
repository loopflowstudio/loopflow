# Task conversation and ordinary background Flows

October 4, 2026. LOO-353, Product. **Design review finished and approved by
Jack Heart.** Jack requested further kickoff reconciliation and continuation
through the existing saved pursue Flow. Reconciliation is complete; automatic scheduling and the Ready/Complete review
control path are deleted locally. Task-worker authority and the remaining runtime
cut and configured acceptance are outstanding.

## Outcome

One native Task conversation retains design/review, files, drafts, shells and
layout while background Runs execute operational Flows. The configured demo
remains unexecuted.

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

The initial audit inspected checkout `2f14e4422` and local main `c5dc238b0`,
without fetching, launching or migrating. Prior scratch is preserved at
`6513477a5`; the superseded worker-start repair proposal is at
`5090f672e:scratch/task-workspace-continuation.md`.

The checkout includes local main `58d3b5b3e` through merge `0ea8de07b`. Local
implementation removes repository-driven Flow restart scheduling, its five
counters/columns and automatic enrollment. Task
and PR checks share delivery reconciliation and per-landing locks; it removes
the redundant repository scheduler lock. This is local source evidence, not a
remote-tip or complete worker-removal claim.

Integrated main also carries Desktop launch/refresh timing and worktree-list
performance changes. Preserve Session fencing outside async runtime waits and
the confirmed-dead completed-provider exemption in Task admission; live or unknown
providers still block. `b908182f5` adapts that exemption's test to the removed
recovery mode. Infrastructure's older managed restart/review repairs explain
retained process/effect evidence, not authority to restore those controllers.
Product has no child Wave memory files in this checkout.

The latest main change records LOO-292's published-installation and checkout-sync
evidence only; it changes no runtime code and proves no LOO-353 provider behavior.
Installation promotion and checkout integration remain separate operations.

Preserve main's repo/Wave primaries, Ask removal, native conversation lookup,
Session working-set filtering, configurable New Session and Task history filters.
Task primary selection remains absent. Reuse the existing engine traversal,
primary scope lock, indexed Flow inventory/lazy detail and Swift graph renderer.
The deletion inventory below identifies their controller dependencies. Claude's
persistent stream-json stdin is not native-composer delivery and is unnecessary
for workflow navigation.

Inspected `lf/mod.rs` still exposes `--mode batch` without ordinary `-b` detachment.
`ops/flow_run.rs::exec_driver` and its invalid `-b` call are already deleted.
Add ordinary detached skill/prompt/Flow launch through one
placement/Exec path, returning durable Session/invocation identity after admission.
Batch selects provider behavior; detachment is separate. Delete the worker path.

October 4 implementation deletes new review reservation, `serve-flow`, its capture
environment, review launcher/checkpoint push and store publication APIs. Operational
launch rejects human steps before capture, recursively through resolved subflows and
XOR paths. The draft records each legacy review as a Session observation and converts
its kind to Conversation, preserving identity, completion, pending-boundary links,
captures and effect history. Read-only historical review projections still exist.

`TaskWorkerClaim`, Task `__worker`, managed selection, `recover_flow`, `retry_flow`
and worker control columns still execute. Their store/driver removal remains the
next runtime cut; deleting the review branch is not complete worker removal. Public
saved Flow resume, Ready/Complete, `exec_driver` and completion-only `stop-client`
are already deleted. Compression also removes review launch tokens, captured-skill
overrides of ordinary discovery, duplicate review open/rename dispatch, and the
unused waiting-review store queries. Open and rename use stored conversation identity;
native process fencing and captured history remain. Worker-exclusive controller tests are retired with the review
path; ordinary Flow tests carry execution and crash/effect proof.

Attention still projects Review/Reply and `--needs-me`; Task primaries, workflows,
autonomous loop syntax and ordinary detached launch remain unimplemented. Current
human-bearing templates must become conversation workflow guidance before release;
they now fail operational launch. No live Home or saved invocation was changed.

[Workspace review](../docs/reviews/task-workspace.md) and Unit 3 at
`bc78c27c017bc93099c06fd342b51bc6110beb5d:scratch/growth-thoughts.md` retain
native/remote/file/performance obligations. Their Ask and switching requirements
are superseded. PR #1313/tag `backup/file-browser-20260930` retains web-prototype
evidence, not another editor to restore.

## Chosen architecture — reconciled October 4

### Human workflows and operational Flows

Author workflows in `.lf/workflows/<name>.yaml`: conversation-stage keys with
optional review skills, and edges with `from`, `to`, `flow`. This reversible source
format adds no Work kind, runtime row, shared cursor or boundary token. The native
Task conversation interprets Jack's direction and launches authorized edges via
`lf -b`. Reopening reads conversation and work history. Display guidance without
a fabricated current-stage marker; reuse catalog/source resolution and graph
primitives, never compile workflows into executable Flow graphs. Missing/invalid
references stay visibly invalid.

Operational Flows compose autonomous work, loops and XORs only. Validate resolved
composition before launch, rejecting human steps/workflows even inside nested
references or XOR alternatives. Review skills remain conversation instructions.
Each edge launch creates a distinct captured FlowSession linked to Execs and
AgentSessions, ending completed, failed or blocked with exact results/effects.
Completion supplies evidence, never conversational navigation authority.

For example, design → delivery review and delivery review → delivery review may
both name `pursue`; each choice launches fresh work. Questions, holds, unrelated
messages or ambiguous “yes” across results launch nothing. Stop targets an exact
owned process; later continuation requires inspection and explicit fresh launch.
Publication/merge still require applicable authorization.

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

Retain captures, events, attribution, exact Session/Exec links and effect receipts.
Ordinary launch admission and process fencing prevent duplicate admitted launches;
Task membership grants no execution authority. Publication/landing retain their
own idempotency. There is no feedback transaction, automatic resume, sleeping
runner, segment token, start-after-human API or composer injection. After a crash
or uncertain remote liveness, the caller inspects execution/effects before fresh
work; missing evidence stays unknown. Migration preservation is specified below.

### Current source and launch bridge

October 4 inspection: local main `a1d2f8a59` has zero-human `pursue`, ending
at `pr-publish`. Saved capture `fbd24356-3d6f-415d-a236-16b4f6140c8f` instead
has implement → compress → refresh → loop-decide → pr-publish → demo → loop-decide.
At inspection it was at node 0, with no completed nodes, pending Session or failure.

Jack requested continuation of that existing capture without replacement. Its saved
shape is historical execution evidence, distinct from current source templates;
no fresh invocation status was read during this reconciliation. The bridge retains
no worker authority in the final product and authorizes neither live migration nor
replacement of uncertain work. Workflow navigation remains with the caller.

### Primary selection and Waiting

Extend existing primary selection with `PrimaryScope::Task(TaskId)`. Ensure under
the existing scope lock; reuse an explicitly chosen existing Task conversation or,
on initial selection, the sole unfinished interactive Task conversation. If several
exist and none is selected, default to the most recently used interactive conversation
and preserve the rest. Reuse the ranking in `human_session::latest_interactive_session`: native
human input, then per-Session interactive opening, then creation time. Assistant
output must not change the choice. Reuse the ranking with Task-membership and
unfinished-interactive eligibility; ordinary `lf resume` also admits completed
conversations, so its candidate set cannot be adopted unchanged. An explicit
primary remains selected regardless of later recency. This is a reversible
implementation default, not evidence of Jack's preference. If none exists, explicit Task conversation opening creates one
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

The local cut deletes `ops/human_session.rs::complete`, including ordinary
provider termination and review settlement, and removes the CLI, store and Desktop
completion consumers. `exec_driver` and review-completion relaunch are deleted.
Historical feedback cannot return an executable Flow result. Exact-client stop/interrupt and
reversible Close view remain; conversations stay reopenable. Add no finish,
resolve or archive-and-stop replacement. Primary replacement retains its own
successor/history semantics, never review settlement.

Compression also deletes the orphaned `session stop-client` command and its
completion-specific shutdown helper. Primary replacement uses the existing native
client replacement path; its successor/history semantics and legacy stop receipts
remain. Native move tests retain unknown-exit, exact-client and delayed-history
coverage. Task-worker claims and managed review launch remain deletion targets.

Compression removes the unused feedback payload from `SkillOutcome::Completed`
and the duplicate feedback field/query from `FlowSession`. Archived feedback is
read through Session evidence only; autonomous decision direction remains intact.
Builtin operating guidance no longer prescribes the deleted review handshake.

Preserve historical `completed_at` and feedback so completed records do not
reappear as active. The single draft now copies `ready_summary` into an immutable Session observation
before dropping its live column. Reads retain historical feedback; Ready state and
completion actions are deleted. Imported historical feedback uses the same event
representation and grants no attention or navigation authority. Provider Completed and Run
outcome remain execution facts. Stopped clients remain accessible in retained/
history views without completion or pane closure erasing them. Presentation-only
archive is outside scope. Existing Task completion keeps unresolved feedback and
unknown execution explicit; it must not complete every associated conversation.

## Template and consumer cutover

Move kickoff/review and demo/revision into workflow conversation stages. Project's
Default Flow selects operational work. Remove post-review deciders and all
`human: true` execution paths from new definitions. Keep autonomous XOR selection.
Every associated Flow gets equally rich lazy graph/progress/output views from
Rust captures and history. Workflow guidance stays separate; selection grants no
process authority.

## Delete — do not maintain

The architectural cut is indivisible; these are removal targets, not components to
modernize first. Paths below are under `rust/loopflow/src/` unless qualified.

- `durable.rs::{TaskWorkerOwner, TaskWorkerClaim, TASK_WORKER_CLAIM_ENV}` and the
  mutable controller portion of `FlowSession`/`FlowTurnSelection`; retain exact
  Session completion selection and ordinary process/Session fencing.
- `tasks.current_invocation_id`; `store/flows.rs` and `store/sqlite/flows.rs`
  managed selection, `start_task_flow`, `claim_task_worker`, `handoff_task_worker`,
  `reclaim_task_worker`, `checkpoint_flow`, `retry_flow`, `recover_flow`,
  `reserve_task_review`. `complete_task_review` is deleted. Replace mixed history writes at
  their owner, rather than deleting their retained data.
- `ops/run.rs::TaskWorkerExec`, `controller/task` worker/review progression,
  Task `__worker` and managed
  branches in `lf/commands/flow.rs`. Keep shared engine graph/cursor traversal.
- Review reservation/launch in `ops/flow_session.rs`, `serve-flow`, review capture
  reservation/publication and automatic review checkpoint/push are deleted. The draft
  converts legacy review kinds without resolving their boundaries. Review launch tokens,
  skill overrides and duplicate open/rename paths are deleted, including `ops/flow_session.rs`;
  managed review projections still need the worker cut.
- Ready/Complete and hidden `stop-client` CLI handlers, store mutations, Session action/Ready state,
  Desktop completion controls and pane-resolution callback are deleted. Historical
  feedback remains in the read DTOs, sourced from Session evidence. Generated
  instructions and affected docs no longer prescribe those commands. Preserve
  provider Completed, historical completion timestamps and primary-successor
  bookkeeping. New review reservation/serve-flow and legacy kind conversion are cut;
  review-token/open/rename dispatch is deleted; managed projection and restart
  consumers still need deletion.
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

Saved `flow resume` and its exclusive recovery assertions are deleted. Internal
Task retry, worker claims, store recovery and managed driver paths remain removal
targets; deleting the public command does not remove their authority.

One draft migration, generated with `uv run python scripts/new_migration.py task_flow_observations`
converts the released schema directly; it removes five scheduling columns and
archives review feedback before dropping the live readiness column, and converts
legacy review kinds to conversations with unresolved-boundary observations. Extend
that same draft for worker-control deletion. Preserve every invocation,
Session, event, exact review and effect receipt; archive last legacy position as
historical evidence, not resumable state. Existing pending reviews remain visible
as unresolved historical boundaries attached to their original conversations; they
are not auto-approved, discarded or silently reassigned. No live Home conversion
or active worker interruption is authorized by this design. Conversion requires
confirmed quiescence of writers in that Home; an isolated copy proves migration.
Keep one executor after cutover. Caller inspection cannot settle or silently
replay legacy reviews; live rollout remains separately authorized.

Review rejected handshake-like feedback receipts. Superseded proposals remain at
`5090f672e:scratch/focus-on-your-own-work.md`.

## Internal slices and remaining workspace scope

One coherent runtime/UI change, with no intermediate release of two executors:

1. **Runtime slice remains:** current local main is integrated. Repository checks
   now reconcile deliveries only; Task and PR checks share `pr_landing`'s finite
   reconciliation entry point and per-landing locks. The Task scheduler lock,
   automatic Flow scheduling/enrollment, five scheduling columns and their DTO/UI
   consumers are deleted. Review traced
   CI enablement into implicit Flow creation and removed that side effect, plus
   the now-unreachable managed recovery exception in repair admission. CI repair keeps
   its existing per-Task hold and incident retry policy. Direct Task-worker claims
   and internal Task recovery still exist; public saved Flow resume is deleted.
   Ready/Complete store/CLI/UI paths, `exec_driver`,
   review-completion restart and feedback-to-Flow-result consumption are deleted;
   new review creation/serve-flow are deleted and legacy conversion is implemented.
   Remove Task-worker
   authority at store/driver ownership while preserving invocation/event/effect
   history and cutting over the minimum DTO consumers. Build the changed Rust code
   and retain the focused ordinary-Flow crash/no-restart case in `flow_tests`. It proves
   repository inspection does not restart a killed ordinary Flow before or after
   its surviving effect receipt. It does not prove Task-worker removal, detached
   admission or delivery-effect deduplication. The populated migration case separately
   proves review-kind conversion while retaining unresolved links and effect history.
2. Separate workflow source guidance from executable operational graphs; implement
   canonical autonomous loops/XOR composition and the loop-or-next catalog rename.
   Review-specific tokens, launch reads and duplicate open/rename dispatch are deleted;
   prune remaining managed projections. New review launch and settlement APIs are deleted. Add
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

Chapter KRs still require Jack's dated confirmation: three consecutive days mostly
in Desktop, and three two-hour sessions without crash/lost access forcing a move.
No metric targets are supplied. Local checks earn neither KR completion nor
Cube/Etude/Kata/Hootro weekly-progress credit.

Checks: `cargo build -p loopflow --bin lf`; seven focused `cargo test -p loopflow` cases (five in `--test flow_tests`: operational review rejection, ordinary crash/effects, authored execution, inspection without Started, bound contributions; Session CLI contract; populated draft conversion); `cargo clippy --all-targets -- -D warnings`; `cargo fmt --check`; `git diff --check` — passed. `lf context --skill implement`: memory 15,962/16,000 and scratch within budget; stored goal exceeds its source budget by 1,276 tokens and remains excerpted, with authored steers unchanged. Compression recheck: `cargo test -p loopflow --lib -- ops::human_session lf::discovery lf::commands::run` and `--test session_cli_tests` pass with inherited `LF_FLOW_STEP`/`LF_RUN_*` unset; inside a Flow step two `lf::commands::run` cases read the ambient step token and fail (hermeticity gap, owner: gate). Worker/store deletion, final schema cut, gate and configured demo remain outstanding; no live Home or saved invocation was mutated.
