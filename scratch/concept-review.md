# Concept review: Flow execution and Wave attribution

Working human review, 2026-09-25, LOO-295. This records the current conversation,
not implementation approval or runtime acceptance. Earlier proposals remain in
Git history and the linked evidence notes.

## Current human direction

A loopflow remains a Flow with backward edges. Task execution is the present
UI focus. A Flow can also run *about* a Wave; attribution does not require a
Wave-owned execution lifecycle or a new Wave loopflow UI. The earlier
anywhere-Flow direction must not be interpreted as a requirement to extend
obsolete Wave machinery.

The human describes Wave playhead as mostly old code that should probably be
deleted, though a similar capability may return someday. Treat deletion as a
candidate simplification. Do not retain or extend it for hypothetical future
use. The human subsequently authorized straightforward deletion and
simplification alongside this work, while deferring investigation of unused UI.

## Proposed usage and skill guidance

```sh
lf task run LOO-295 --flow feature
```

Review the design, then let implement → compress → review-slice → concept-review
→ loop-decide traverse the work. Iterate carries direction back to implement;
Advance reaches the human demo. At demo, human Iterate revisits implementation;
human Advance continues to the declared suffix or finishes the Flow. Delivery
is explicit; the feature Flow does not itself grant merge authority.

Resume the same saved invocation after interruption. Preserve the captured
steps, direction, accepted decisions, and exact human boundary. Blocked opens
one Ask running unblock; human completion returns evidence for reassessment
without approving another Flow gate. These are the intended interactions;
source and fixture evidence do not establish the live end-to-end experience.

Skill guidance: operate on the selected invocation and its supplied decision
protocol. Attribution supplies context. Work and review skills supply evidence;
loop-decide records Advance/Iterate or requests help. Continue using lf's
Codex/Claude Code harnesses. Jev/Pydantic informed the typed protocol and
validation model; neither is a new runtime dependency.

## Core model

| Experience | Current representation / API |
| --- | --- |
| Choose the work sequence | Flow / ConcreteStep and RepeatPolicy |
| Continue the same Task attempt | FlowPosition with captured QueuedInvocation |
| Track passes and direction | FlowProgress: per-edge counts, direction, pending verdict |
| Choose a path | FlowVerdict with FlowDecision::Advance or Iterate; lf flow decide |
| Ask for missing input | lf flow blocked → keyed Ask running unblock |
| Authorize a human boundary | Exact Flow Session Advance/Iterate |
| Interpret an edge | finish_step; persistence and authority belong to its caller |

## Observed code and cleanup implications

The Wave resident still reads context.playhead in runner.rs::run_pass. Runtime
ensure_playhead loads the root wave Flow; Playhead::finish_body increments its
cursor, and settle wraps the root. These are reachable source paths in this
checkout, not evidence that the human wants this model retained, nor a live UI
inspection. Remove the prior requirement to add loopflow semantics to them.

Task execution imports QueuedInvocation and StepRef from controller/wave/playhead.
FlowPosition stores the former and exposes the latter; TaskExecutionSnapshot
also uses StepRef. Deleting the Wave interpreter therefore requires preserving
these still-used Flow representations under their proper owner. Saved Wave
journal state also needs an explicit disposition. Removing a module blindly
would lose more than the obsolete concept.

Separate remaining findings:

- Task loading rejects XOR. This still matters to Flow composition; the Wave
  clarification does not resolve it.
- Ordinary XOR captures a selected body but loads router/branch content when
  entered. Whole-invocation definition pinning remains incomplete wherever
  those paths are supported.
- The authoring and CLI docs mixed the old Task-only verdict protocol with
  the new protocol. This review removes those instructions, assigns decisions
  to loop-decide, and retains explicit Task XOR and pinning limitations.

## Deletion opportunities and proposed disposition

Human direction: aggressively identify cleanup enabled by this change. A large
deletion may become a bounded follow-up, but deferral must name what disappears,
why it is separate, and the proof that closes it. Future usefulness is not a
reason to preserve unused machinery. Apply this scrutiny to new branch code as
well as inherited code. Broader dispositions below remain recommendations.
Local cleanup removes the redundant flow_session::revision_target wrapper;
callers use the shared human_revision_target directly. No external Tasks have
been created. Wave and UI removal remain follow-up proposals.

| Candidate | Delete or consolidate | Proposed disposition and proof |
| --- | --- | --- |
| Obsolete protocol documentation | Remove Task-only repetition claims, concept-review decision ownership, and continue/complete/blocked command examples from docs/authoring.md and docs/lf.md. | This branch: one usage account matching the accepted protocol and explicit remaining limitations. Search all shipped docs/skills for competing instructions. |
| Wave Flow queue | WaveRuntime::enqueue_flow has no caller in the searched source. Playhead::enqueue and its queue/return bookkeeping support the old sequencer. | Wave cleanup follow-up if removal crosses the resident rewrite. Remove the unused entry point, then remove queue state only with a disposition for journaled continuations. Preserve readable history; prove no queued work silently disappears. |
| Wave interpreter and its surfaces | Root wrap and frame settlement in playhead.rs; ensure_playhead and restart_legacy_playhead; __flow-step dispatch/run_step; Wave --restart-flow; /playhead, broadcasts, and their client projections. | One bounded Wave simplification: schedule the existing bounded wave/operate behavior without the obsolete Flow sequencer. Preserve cadence, chat, live provider ownership, interruption and visible failure/recovery. Verify the real consumers before deleting projections. |
| Task types housed under Wave playhead | QueuedInvocation, StepRef and StepKind are used by durable.rs, controller/task/mod.rs and ops/task_execution.rs. QueuedInvocation itself stores identity, name and steps, not a queue. | Move the required Flow representation to its actual owner as part of deletion, with a name matching its contents. Do not copy it into a second implementation. Prove existing Task positions and exact human tokens still decode and resume. |
| Duplicate continuation logic introduced or retained here | Task drive_task/decide_human_flow_step and ordinary execution/flow_session::next_cursor each manage traversal and human revision around finish_step. | Review in this branch while resolving Task XOR: consolidate traversal and pure human navigation where possible. Keep Task transactions, claims and domain effects at their owner. Do not merely wrap both existing interpreters or delete standalone support because its UI is absent. Proof: identical authored paths, directions, limits and stale-decision outcomes through both adapters. A larger persistence unification needs its own bounded design. |
| Shared scratch route protocol | build_xor_routing_suffix and read_xor_verdict still use scratch/route-xor.md outside the exact navigation protocol. | Pair removal with XOR/pinning repair. Persist an exact route selection through the existing invocation authority, then delete the shared file protocol and source reloads on recovery. Prove stale/concurrent/nested route isolation and recovery after source deletion. |

Source evidence: the builtin wave/flow/wave.yaml contains only wave/operate and
assigns recurrence to the scheduler. The old resident still consumes playhead
state, so removal is a behavior-preserving simplification, not a claim that the
whole module is dead. Swift WaveChatClient decodes PlayheadView, and
AttemptFailurePresentation consumes it; preserve useful failure presentation
without preserving the obsolete execution concept. No live UI was inspected.

Keep historical decoding only where recoverable persisted facts require it.
The old decision aliases and migration readers cannot be deleted merely because
their names are obsolete. Establish migration/disposition first. Likewise, the
new ordinary FlowRun and FlowSession records must earn their shape, but ordinary
Flow execution and Wave attribution remain supported goals.

Deferral should leave a task-ready directive containing the deletion boundary,
surviving behavior/data, prerequisites and completion proof. Do not make a vague
"cleanup later" item. Large Wave removal may be separate from LOO-295; obsolete
instructions and duplicated semantics touched by LOO-295 belong in its review.

## Next proof

Defer the Wave/UI usage inventory. Its future deletion work must preserve
Wave operating/cadence behavior and Task recovery without introducing a
replacement Wave Flow lifecycle solely to make deletion fit.

The next product choice is the pass budget after human revision. The current
reducer retains per-edge counts for the entire invocation. If loop-decide
uses its seven backward traversals, Advances to demo, and the human requests
another implementation pass, loop-decide has no remaining Iterate allowance.
The human demo edge has its own counter; it does not replenish loop-decide's.
This is source-observed behavior, not an accepted decision that human revision
must share the original autonomous allowance. Decide whether a new human
revision starts a fresh bounded work cycle before changing persisted counters.

Task acceptance still needs the concrete interaction and recovery proof:
initial steps once, backward traversal with direction, later forward completion,
exact human approval, and Blocked → Ask → reassessment. Preserve stale-decision
rejection and saved-result recovery. Existing fixture reports are in
protocol-review.md; no tests or live runtime demo were run during this review.

Local cleanup verification: cargo fmt --all -- --check, all-target Clippy with
warnings denied, and git diff --check passed. The Rust edit only removes a
forwarding function; no behavior change or new test is claimed. Source review
confirmed the caller uses the same shared revision-target function directly.
