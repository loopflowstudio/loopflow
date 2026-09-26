# Loopflow entry points and counterexamples

Read-only source audit, 2026-09-25, for LOO-295. The governing direction is
`scratch/loopflow.md`: a loopflow is an ordinary Flow containing backward edges,
accepted everywhere a Flow is accepted. Task ownership supplies context and
delivery authority; it must not supply different transition semantics.

This audit changes only this document. No runtime, provider, migration, or test
was executed. Findings describe the working tree inspected during concurrent
implementation, not deployed behavior. Rust paths without a repository prefix
are relative to `rust/loopflow/src/`; other paths are repository-relative.
Symbols are more durable references than line numbers on this active branch.
Runtime files changed during the final reference check (including the Task
progress representation), so these observations are the pre-redesign integration
baseline, not a review of the main agent's subsequent implementation.

## Entry points and actual executors

| Surface | Exact implementation path | Current behavior and integration obligation |
| --- | --- | --- |
| `lf NAME`, `lf flow NAME` | `rust/loopflow/src/bin/lf.rs::{run_target,run_target_in_repo}`, `lf/discovery.rs::discover_target`, `lf/commands/flow.rs::{run,execute}` | Expands a Flow and calls `FlowEngine::run(items, 0)`. Generic execution rejects repeat. Keep the same dispatch and Flow type; persist invocation state before launching boundaries. Bare discovery prefers skills except special `design`/`launch-plan` names. Explicit Flow kind checking also uses discovery, so test collisions rather than assuming the explicit verb bypasses them. |
| Direct attributed invocation, `lf --task/--wave/--as … NAME` | `bin/lf.rs::{prepare_work_binding,prepare_hierarchical_work_binding,run_bound_target_in_repo}` | Named bound targets explicitly reject `Target::Flow`. The explicit `Commands::Flow` arm instead calls `run_target`; `--task`/`--as` can first establish cwd, while `--wave` alone only enters direct-binding setup for Skill/External commands. Per-skill `lf/commands/run.rs::run` carries subject selectors. Reconcile these dispatch differences; the operating instructions promise bound Flows already. A helper Flow attributed to a Task must have its own invocation and never claim the Task's managed cursor. |
| Managed `lf task run … --flow`, restart, resume, advance, hidden worker | `ops/task.rs::{task_run,select_task_worker_flow_from_project,load_task_flow,launch_task_process,task_advance,resume_task_async}`, `controller/task/mod.rs::{run_worker,drive_task,run_task_with}` | Durable Task invocation and claims, fresh provider Run per Skill. `load_task_flow` rejects XOR, accepting only Skills/Ops. `drive_task` already iterates boundaries within one driver process; older memory saying every boundary launches a new worker is stale for these bytes. Move edge meaning out of this Task-specific reducer, retain Task preflight, placement, PR and atomic domain effects. |
| Scheduled Flow | `ops/cron.rs::{spawn_cron_target,run_cron}` (runner entry is found around the call to `spawn_cron_target`) | Spawns `lf --wave W --batch flow NAME`, so it reaches generic Flow execution. Headless human gates need a durable visible wait. A successful launcher exit must not become a successful completed-flow receipt merely because execution parked. Check receipt semantics when introducing a wait outcome. |
| Wave root and queued Flow API | `controller/wave/runtime.rs::{ensure_playhead,enqueue_flow,restart_legacy_playhead}`, `controller/wave/runner.rs::{run_pass,run_harness_pass,run_process_pass,spawn_wave_step}`, `controller/wave/playhead.rs::{Playhead::finish_body,Playhead::settle}` | A third interpreter exists. Skills execute from the saved Skill definition through a Harness; other steps spawn `lf -b __flow-step FLOW INDEX SEED`. Playhead settlement always increments the index; its root wraps to index zero. Neither repeat decisions nor human policy are consulted by that settlement. The public `enqueue_flow` API had no call sites outside its definition in the searched Rust tree, but saved queues and the root runner remain real execution/recovery surfaces. Root scheduling/wraparound must not leak into finite Flow completion. |
| Hidden `__flow-step` | `bin/lf.rs::Commands::FlowStep`, `lf/commands/flow.rs::run_step` | Reloads the named Flow, extracts one expanded top-level item, then runs a fresh generic engine on that singleton. A backward edge cannot resolve against its preceding siblings there; a source edit changes the definition relative to the saved Wave invocation. This boundary needs pinned occurrence/invocation context if retained. |
| Library engine API | `engine/execution.rs::{FlowEngine::run,run_with_cursor}`, `SkillExecutor` | `run_with_cursor` preserves only caller-owned memory. `ExecutionCursor` is serializable but production CLI callers use `run`, not a durable cursor store. Removing its repeat refusal alone would advance to the next index and ignore the decision. |
| Catalog, authoring and install verification | `engine/flow.rs::{load_flow,expand_flow,human_occurrence_ids}`, `lf/commands/flow.rs::{show,validate}`, `lf/commands/install.rs` catalog load/expansion | These must recognize and validate the same topology as execution. Successful loading is not proof of executability; currently Task, standalone and XOR-inline validation have different coverage. Installation should validate new selections while preserved invocations continue from captured definitions. |

`ops/flow.rs::execute_flow_ops` dispatches explicit operations, not whole Flows.
The current `ops/project.rs` contains planning/preparation helpers and no separate
Project Flow executor. SSH and normal launch wrappers route into these CLI
surfaces; they should preserve selected identity and placement, not interpret
edges themselves.

## Composition is part of the contract

`engine/flow.rs::expand_with_chain` flattens explicit `Step::FlowRef` and implicit
multi-skill subflows into one concrete sequence. `flow_parents` records ancestry;
it does not establish a cursor namespace. `validate_occurrence_ids` requires IDs
to be unique across the expanded sequence and all XOR alternatives. Reusing the
same subflow twice, or in two mutually exclusive branches, can therefore collide.
Decide whether occurrences receive structural identities during expansion; do
not silently make reusable loopflows require hand-renaming their internal IDs.

`ConcreteXor` preserves a router name and `XorPath` references, not recursively
pinned child definitions. `engine/execution.rs::run_xor` runs a router, reads
`scratch/route-xor.md`, remembers the selected branch in `NestedCursor::Xor`, and
calls `load_xor_path_items` every time it enters/resumes that path. The router's
Skill content is loaded at execution too. Thus even the saved outer
`QueuedInvocation` does not freeze XOR descendants or its router.

`load_xor_path_items` has three materially different paths:

- `flow:` calls `expand_flow`, including repeat validation, from mutable source.
- `skill:` loads one Skill with default occurrence policy.
- inline `steps:` resolves Skills and returns their policies without calling
  `validate_repeats` over that sequence. Parsing checks individual policy fields,
  but an invalid/missing/backward-body target is not checked by the outer
  sequence's `validate_repeats`, which skips XOR nodes.

The file-based XOR verdict validates a first-line path name, not invocation,
occurrence, Run, or attempt identity. The inspected router/read path does not
clear or fence an earlier `scratch/route-xor.md`. A stale file containing a valid
path name is therefore a concrete acceptance counterexample. Nested routers
also share that filename. Preserve the selected route as a durable exact decision
before entering its child; recovery should not rerun the router or reread a
mutable shared file.

Current repeat validation permits disjoint sequential sections. It rejects a
body containing human nodes, Ops, XOR, or another repeat reviewer. Those are
existing restrictions, not accepted product requirements. At minimum a valid
loopflow used *inside* an XOR path must work. If repeating *around* an XOR or
nested/overlapping loops remains unsupported, say so as an explicit grammar
decision and reject it uniformly before any prefix executes.

## Persistence, authority and recovery

The durable implementation is currently Task-specific:

- `durable.rs::FlowPosition` owns a `TaskId`, saved `QueuedInvocation`, scalar
  step index and iteration, version, worker generation/claim, Session binding,
  blocker and optional `LoopReview`. `QueuedInvocation` lives in
  `controller/wave/playhead.rs` and contains saved concrete steps and UUID.
- `store/sqlite/durable.rs` stores `task_flow_positions`; the draft
  `task_loop_review__bf1c60e59dcb727768e5f27b6bdc120a.sql` adds `review_json`.
  `record_review_verdict` requires ready Task Work and the current claim's bound
  Run, requires a repeat occurrence and nonempty evidence, accepts identical
  retries and rejects conflicting saved verdicts.
- `ops/task.rs::task_verdict` resolves Task identity from the checkout and the
  active Run environment. It cannot serve an unbound invocation, and attribution
  to an existing Task must not authorize a helper's verdict against that Task's
  managed Flow.
- `controller/task/mod.rs::finish_task_flow_turn` consumes Continue/Complete/
  Blocked. Continue carries direction, increments the active edge's count and
  jumps to `from`; Complete clears the review and moves forward; missing output,
  Blocked, and exhausted budget fail rather than completing. The global iteration
  is separately used in occurrence/human identity.
- `store/sqlite/children.rs::{settle_task_worker,finish_task_flow}` atomically
  fence claim/version and update Task evidence; final settlement removes the
  position and records `TaskEventKind::FlowFinished`. A generic refactor must
  retain these domain effects without making Task rows prerequisites for ordinary
  Flow execution.
- `ops/task.rs::launch_task_process` joins an exactly live claim, reclaims a
  proven-dead owner, and refuses unknown owner evidence. `drive_task` checks its
  launch claim before each boundary. Invocation ID, position version and worker
  generation fence different races; Run attribution alone is not a claim.
- A saved verdict takes the `has_pending_review` path through
  `run_task_op_boundary`, consuming it without another provider call. Explicit
  interruption and `release_task_worker_in` clear a pending verdict; reclaim and
  saved-result recovery are different transitions. Preserve that distinction
  deliberately, including death after save but before provider termination.

One optional `LoopReview` is sufficient for the presently disjoint loops only
because Complete clears it before entering another edge. It is insufficient to
remember two simultaneously active nested loops. Direction lifetime, budget
reset on re-entry, and whether human revision consumes a loop pass all require
defined semantics; do not reuse worker retry generation for pass count.

Generic `lf/commands/flow.rs::execute` discards its cursor on return and maps both
`FlowOutcome::Waiting` and `Completed` into the same completed journal event.
The production executor currently always returns Completed after a successful
Skill. Adding durable waits without fixing this mapping would produce false
completion evidence. Run history/journal events are evidence, not an existing
generic mutable continuation record.

Ops need their own recovery consideration: Task and generic paths execute the
external operation before cursor settlement. A crash in that gap cannot be
solved by replaying a pure reducer. Use the operation's existing idempotence or
receipt contract; never infer exactly-once PR/delivery effects from exactly-once
cursor settlement.

## Human boundary paths

Standalone `lf/commands/flow.rs::CliFlowExecutor::run_skill` rejects a human
occurrence in a noninteractive run. In an attached run it executes the provider,
checkpoints, then asks `confirm_present_human_review` for terminal acceptance.
EOF and refusal fail. This has no durable Session, exact resumable approval
receipt, Iterate transition, or post-crash continuation.

Task human boundaries use `ops/human_session.rs::{prepare,serve_flow,open,decide}`.
`FlowSessionToken` includes Task, invocation, Flow, node, saved Skill and iteration.
Session IDs encode Task/invocation/Flow/node/iteration. `mark_ready` checks the
current token and bound human Run but does not advance. `decide` commits the
decision through `controller/task/mod.rs::decide_human_flow_step`, tries successor
launch, then stops the old review Run. Approval now precedes teardown in the
inspected tree; older Task seed prose describing the reverse is historical.

The store's `finish_human_task_boundary` and human settlement compare the exact
unclaimed position in a transaction. An old Session ID cannot approve a new
review. `active_flow_skill` restores the saved Skill into the review provider.
The conversational prompt permits Advance only after explicit human direction;
neither readiness nor provider exit authorizes it. The autonomous reducer must
not obtain approval simply by encountering a human node.

`lf session advance/iterate` (`lf/commands/session.rs::decide_flow`) and
`lf task advance --session … --summary …` (`ops/task.rs::task_advance`) converge
on this Task-specific decision path. Session listing currently enumerates
`human_task_flow_positions`; standalone waits would be invisible until this
projection understands generic invocation identity. Existing Ask records and
ordinary interactive Runs have different Complete semantics: neither is a
substitute for an exact Flow approval.

Iterate currently picks `preceding_autonomous_step`. At demo after pursue this
selects concept-review rather than implementation. Share an authored revision
edge/target with both execution paths, or explicitly resolve the repeat topology;
do not retain a Task-only nearest-Skill heuristic. Preserve distinct design-gate
revision behavior.

The desktop is a client of the CLI Session surface, as shown by
`swift/LoopflowTests/SessionsStoreTests.swift` and
`swift/LoopflowMac/Views/SessionsView.swift`; no desktop scheduler is required.
Extend shared records and projections so app close/reopen cannot lose a wait.

## Minimal shared transition seam

This is an interface suggestion, not a chosen storage redesign:

1. Compile one immutable Flow invocation, including composed occurrences, XOR
   router content and every reachable child definition. Give each occurrence a
   structural identity that survives source deletion and distinguishes reuse.
2. Share one pure transition function conceptually shaped as
   `reduce(saved_definition, saved_progress, exact_event) -> transition`.
   Events distinguish ordinary boundary completion, autonomous Next/Repeat/
   Blocked, selected XOR route, exact human Advance/Iterate and interruption.
   Output identifies the next executable occurrence, waiting human boundary,
   blocker, or finished invocation. It must not launch a provider or authorize
   an external operation.
3. Persist decisions before reduction/settlement; fence writes with invocation,
   occurrence/visit and attempt authority. Atomically consume the accepted event
   and update progress. Keep pending decision and pass progress conceptually
   distinct, even if serialized together. Invalid output leaves a named stop.
4. Let existing drivers execute the returned boundary and let their storage
   transactions own receipts. Task contributes its Work/PR/events; standalone
   needs durable invocation lookup and a resume path without inventing Task Work.
   Do not maintain both an engine cursor and a Task cursor as independent owners
   of one invocation. Wave hosting/queues may remain scheduling context, but
   must delegate invocation transitions rather than independently incrementing.
5. Generalize the current exact decision endpoint and Session identity around
   the invocation. Task selection may resolve its current invocation, while a
   helper or unbound Flow names its own. Sharing topology does not merge human
   approval authority with reviewer capability.

The main implementation must choose standalone persistence/resume ownership.
This audit does not prescribe a new scheduler, resident, Flow type, or table.
Any migration must preserve pinned definitions, pending decisions, claims,
historical human stops and completion receipts; mutable catalog re-expansion is
not a migration of saved execution.

## Concrete acceptance matrix

Run the same pinned definitions through ordinary named/explicit Flow execution
and managed Task execution, comparing occurrence/decision traces, then exercise
the other routing surfaces above. These are proposed proofs, not recorded passes.

| Case | Required observation |
| --- | --- |
| Two sequential backward edges | `init, a, ra, a, ra, middle, b, rb, b, rb, final` for one Repeat then Next at each reviewer. Init/middle/final run once, second edge gets its own full budget and direction, every provider boundary gets a fresh Run. |
| Flat composition | Embed a loopflow between outer prefix/suffix using both `flow:` and supported implicit subflow syntax. Repeat resolves to the composed occurrence; outer prefix/suffix do not repeat. Test two occurrences of the same subflow and establish deliberate identity behavior. |
| XOR composition | A chosen named child Flow with two loops executes and returns to the outer suffix; an unchosen branch never runs. Repeat with inline path steps, empty path, and a human node inside the chosen path. Outer and child cursor survive recovery. |
| XOR decisions | Old valid `route-xor.md`, missing output, invalid route, nested router and two concurrent invocations cannot select a route for the wrong occurrence. Death after a saved route never reruns its router. |
| Definition pinning | Delete/edit outer Flow, subflow, router Skill and chosen child Flow after selection. Recovery, human reopening and later loop passes use captured bytes; a new invocation uses new bytes. |
| Exact autonomous decisions | Missing/malformed/blocked verdict and pass exhaustion never run final steps. Duplicate identical verdict consumes once; conflicting verdict, helper Run and stale old attempt are rejected without mutating progress. |
| Recovery windows | Interrupt before decision, kill after decision save, kill after cursor commit before launch, kill while a provider is still live. Resume retries/consumes/joins as appropriate; no duplicate provider boundary or lost direction. Unknown liveness stays explicit. |
| Exact human decision | Headless standalone parks visibly; attached standalone and Task require the same durable approval. Readiness, terminal close and provider completion do not advance. Reopening after process/app restart preserves the exact gate. Repeated approval for an old visit cannot accept the next visit. |
| Revision target | Delivery Iterate returns to implementation and traverses both reviews before demo; design Iterate returns to its declared design target. Direction survives recovery and does not accidentally consume a worker retry as a loop pass. |
| Bound helper isolation | `--task`, `--wave`, `--as`, bare name and explicit `flow` agree on semantics. A direct Task-attributed Flow can repeat while leaving the managed Task cursor untouched. |
| Scheduled and hosted invocation | Cron preserves waiting/blocked/completed distinctions. Wave runner or queued invocation uses the same edges; root scheduling never causes a completed finite Task/standalone Flow to restart. |
| Delivery and completion | Final Skill/Op runs on Next exactly as declared. No merge or Task completion is inferred from loop completion. Restart after completion reports its receipt and does not select another Flow. Test operation effect/settlement crash windows separately. |
| Migration | Populated prior store and Wave journal retain saved definitions, pending verdicts, human token identity and old completed history. Unsupported historical state gets explicit disposition without catalog substitution. |

Existing tests worth extending selectively: `engine/execution.rs` has
`one_shot_execution_rejects_repeat_before_running_any_skill` (obsolete contract),
`engine_resumes_nested_xor_after_waiting_skill` (memory-only cursor proof), and
`engine/flow.rs::repeat_requires_an_earlier_autonomous_body_and_a_bounded_reviewer`.
`controller/task/mod.rs::driver_runs_fresh_slice_turns_until_the_flow_finishes`
uses simulated provider/Linear transport and truncates pursue before human
delivery; it cannot establish standalone, XOR, live human, or deployment parity.

The smallest decisive proof is a shared transition test with two disjoint edges
and exact saved decisions, followed by the same composed definition executed
through both real dispatch paths. A generic-engine unit pass alone cannot close
the entry-point, persistence, and human-authority gaps above.
