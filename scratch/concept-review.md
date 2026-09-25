# Concept review: replace Task phases with Flow topology

2026-09-25. Attended review of branch HEAD `f122a264b` against local `main`,
using the new canonical concept-review skill directly from this checkout.
This is review evidence and a usage draft, not implementation approval.

## Intent and proposed usage

Accepted human direction: replace the Task's first/loop/finally arrangement
with one Flow containing backward edges. A loopflow is a Flow, not a new
execution mode. Task context must not determine what step comes next.

Latest human correction: there is no human gate concept. The prior review
incorrectly treated the implementation's separate approval path as a required
product concept. Flow topology consists of steps and explicit edges. Human
participation must not introduce a separate navigation model. How an ordinary
step obtains human participation remains to be clarified below.

Proposed wording for the Task/Flow guides and launcher skills:

```sh
lf task run DES-123 --flow feature
# Design -> human review -> implementation and reviews -> decision -> human demo
# Iterate returns along the current step's declared backward edge.
# Advance follows the sequence. A selected delivery step is an ordinary suffix.

lf task advance DES-123
# Continue the same saved invocation; do not select another phase or Flow.

lf flow decide iterate "Revise the interaction and prove recovery"
# The active deciding step chooses its explicit backward edge.
```

The first section runs once unless an edge explicitly returns into it. The
repeated section is determined by the selected edge. The suffix runs when
execution reaches it; it is not exception cleanup. The chosen Flow determines
whether delivery is included. Current `feature` stops after human demo and
does not include publication, merge, or Task completion.

On interruption, continue the captured invocation at its saved boundary.
Source edits affect new invocations. A saved successful decision is consumed
once; failed Runs cannot advance. Preserve actual human instructions and saved
decisions without assuming the existing gate mechanism is the required model.
Blocked asks for missing input; Ask Complete returns evidence for reassessment.
It does not itself choose an edge. Finishing the Flow does
not select a new Flow or complete the Task implicitly.

Accepted human clarification: backward edges must be explicit and clear in the
Flow schema. Iterate follows the current occurrence's declared edge, whether
the step involves a human or runs autonomously. Without an edge, Iterate is
unavailable. The runtime must not infer a target from preceding skills or Task
phases. The earlier proposal to retain a distinct Advance-only human gate was
the reviewer's assumption and is superseded by the human's correction.

The current schema already expresses this directly:

```yaml
- step:
    id: implement
    name: implement
# Work and review steps go here.
- step:
    id: review_delivery
    name: demo
    repeat:
      from: implement
```

`repeat.from` names an earlier occurrence in the same expanded body. This
illustrates topology only; it does not settle how demo obtains human input.

Keep `task-design` and `pursue` as useful composable Flows. Their names do not
give the Task controller phase semantics. Do not remove a useful composition
merely because it formerly served as a lifecycle phase.

Affected guidance: `docs/authoring.md`, `docs/lf.md`, builtin `advance`,
`launch-plan`, `loopflow`, and `pr-land`. Existing guides already explain most
of this correctly. Revise only the mismatches identified below; this draft does
not claim unimplemented behavior is shipped.

## Double loop: both edges return to implementation

Accepted human clarification: the outer human review returns to implement.
Any revised design should be clear by the end of that review. Returning to
implementation does not imply that the design stayed fixed; the review itself
can change it and leaves the revised artifact and direction for implementation.
This settles the topology, not the remaining interaction mechanism or permission
to implement this review's proposed deletions.

The inner loop implements and verifies the current design. The outer loop
includes human review of the experience and clarification of any design changes:

```text
initial design -> implement -> compress -> reviews -> inner decision
                     ^                                   |
                     +-------------- Iterate ------------+
                     |                                   |
                     |                                Advance
                     |                                   v
                     |                  human review + design clarification
                     |                                   |
                     |                            outer decision
                     +-------------- Iterate ------------+
                                                         |
                                                      Advance
                                                         v
                                                 selected delivery
```

The two decisions ask different questions: does the implementation satisfy the
design, and does that design produce the intended experience? Both backward
edges remain explicit. Human participation can inform either review; it does
not define either loop or create a gate. Blocked/Ask remains available wherever
the current decision needs missing input.

Current pursue already gives loop-decide and demo explicit edges to implement.
That topology matches the clarification: the outer body contains the inner
loop and the human review. Keep concept-review among the autonomous reviews;
the diagram groups reviews without moving it out of the current inner body.
The special human settlement path remains a separate implementation question.

Initial design need not run again. Inner Advance reaches human review; outer
Iterate carries the clarified design and direction to implement, then traverses
the ordinary inner reviews and decision again. Neither loop count implies
completion or a pass limit. No phase owner or additional loop controller is
needed. Test a design change made during human review: the next fresh implement
Run must receive it and subsequent review must evaluate the revised claims,
without silently rerunning kickoff or treating earlier proof as sufficient.

The remaining design question is how review interaction and its decision use
the ordinary step protocol. The separate human settlement code remains a
candidate, not an accepted necessity or an approved deletion.

The human affirmed this double-loop account. Use it as the accepted topology
for the deletion review. Review work produces the clarified design and direction;
the deciding occurrence records Advance or Iterate through the common protocol.
Do not add a Task phase transition or infer a return to kickoff between them.

## Core model

| Concept | Representation and responsibility |
| --- | --- |
| Task | Durable objective, worktree, PR history, placement, steering and execution ownership |
| Captured Flow invocation | One identity and immutable expanded definition, including XOR paths |
| Position | One recursive ExecutionCursor; topology determines forward/backward movement |
| Attempt | Exact Run/claim or boundary identity; successful completion authorizes settlement |
| Human interaction | Input to work or decisions; the current separate gate mechanism is under review |
| Ask | Human evidence returned to the waiting decision agent; separate from navigation |

Current Task and ordinary adapters both call `ExecutionCursor::finish`.
Task SQL transactions and ordinary file/driver locks have different concrete
ownership obligations; sharing topology does not establish that either can
be deleted. There is no evidence here justifying a new universal storage layer.

## Concrete findings and deletion inventory

### 1. Implicit human revision remains executable policy

The target model has no human gates. Uses of that term below describe the
current implementation being challenged, not a retained product requirement.

`engine/transitions.rs::finish_step` calls `human_revision_target` for human
Iterate without an edge. That helper selects the nearest preceding autonomous
skill. Its comment describes old saved gates, but there is no legacy-only
condition. Current builtin `task-kickoff`, `launch-plan`, and `ship-demo` still
contain human gates without edges; `design` is a first-step human gate without
any possible preceding target. Both Session prompt builders advertise Iterate.

Concrete counterexample: `ship-demo` expands `task-gate` to review-slice,
concept-review, gate, then demo. Its inferred revision target is `gate`, not
implementation. A user's revision follows a hidden heuristic absent from YAML.

Required simplification: authored steps use explicit edges when revision is
offered. Delete `human_revision_target` and its special branch in `finish_step`.
Derive offered actions and prompt instructions from authored capability. Resolve
the intended revision behavior of each remaining builtin rather than mechanically
assigning the nearest skill. The same rule applies to human and autonomous
decisions, and to Task and ordinary Flow execution.

Preserve existing captured definitions, positions and recorded decisions.
Historical recovery must not silently invent an edge or re-expand current
source. If a saved Iterate lacks an explicit target, retain its evidence and
surface the unresolved transition for explicit disposition. Do not preserve
the implicit navigation rule as a second executable model. Smallest proof:
human and autonomous Iterate without an edge both fail without changing state;
declared edges take their exact target in both adapters; old saved evidence
remains readable and cannot turn into approval or a guessed jump.

### 1a. Reassess the separate human-boundary execution path

Removing the gate concept reaches beyond the inferred target helper:

- `OccurrencePolicy.human` currently selects a different execution path.
- `CliFlowExecutor::run_skill` parks instead of running that occurrence normally.
- `ops/flow_session.rs` provides separate readiness, approval, settlement,
  launch and recovery for ordinary Flow human boundaries.
- Task human settlement lives in `controller/task::decide_human_flow_step`,
  with `FlowSessionToken`, preparation and approval handling in human_session.
- Session Advance/Iterate commands, generated review prompts and client actions
  expose this special navigation path.

The concrete duplication is visible at `lf/commands/session.rs::decide_flow`:
it selects ordinary Flow or Task-specific human settlement, while
`lf/commands/flow.rs::control` already records ordinary navigation decisions.
Likewise ordinary human `flow_session::settle_saved` is an extra settlement
entry before the common engine resumes. Removing this duplication must preserve
human interaction and its saved result, not replace review with automatic
acceptance. Audit canonical loop-decide guidance too: it still teaches a
separate human gate even though the accepted model has none.

These are now deletion or consolidation candidates, not protected architecture.
If ordinary steps obtain human input through Ask and then use the ordinary
decision protocol, their separate approval/settlement path can disappear. If
steps can instead run interactively, interaction may remain launch policy while
completion/navigation still use the common protocol. These are alternatives;
the human has not yet selected the interaction model. Preserve actual waiting
conversations, decisions and explicit authorization requirements while resolving
that design. Removing a gate concept is not permission to manufacture consent.

### 2. Duplicate kickoff composition survives the phase model

`task/flow/task-kickoff.yaml` is kickoff plus human review. `task-design.yaml`
has the same useful sequence with an authored revision edge. Repository search
found no active source caller of the old name; it remains publicly discoverable
through the generated builtin catalog.

Candidate deletion: retire task-kickoff from the new catalog and use task-design.
Captured invocations already contain their steps, so catalog removal should not
require rewriting their identity. Verify saved human reopening and any user
configuration referencing the old name before treating removal as complete.
Retain ship/ship-demo/task-gate where they express deliberately selected delivery;
their existence alone is not evidence of a Task phase dispatcher.

### 3. The new ordinary adapter retains a nonexistent unsaved mode

`lf/commands/flow.rs::CliFlowExecutor.invocation` is `Option<String>`, but its
only constructor is `drive_saved`, which always supplies Some. This keeps
optional boundary creation, conditional checkpointing, fallback Completed,
and a missing-invocation human error alive without a production caller.

Make invocation required and delete those branches. The early human return also
makes later `!skill.policy.human` conditions redundant. `FlowEngine::executor`
has no repository caller. These are local reductions, not a storage redesign.
Proof: ordinary saved execution, op receipt recovery, and human wait tests.

### 4. Obsolete instructions still teach removed behavior

- Builtin `ops/skill/pr-land.md` describes Task finally and earlier-phase rules.
  Delete that explanation. `ops/land.rs::prepare_pr` now checks explicit
  completion plus discardable successor facts, not a Task phase. Keep its real
  merged-PR recovery behavior: explicit PR rotation can still produce that state.
- `work/task/mod.rs::CiCheck::land_time_precondition` explains scratch using
  first/loop. Replace those terms; keep the check's valid delivery distinction.
- `.lf/skills/ci-proof.md` says to put the decision on the final review. The
  dedicated loop-decide occurrence now owns it. Retire that contradictory rule.
- `.lf/skills/xor-route.md` is an old copied gate prompt ending in the removed
  `scratch/route-xor.md` protocol. No repository Flow names it as a router, and
  default XOR routing now captures its own prompt. Delete this unused artifact
  after checking local configuration; do not revive the shared file protocol.

### 5. Completed live-demo scaffolding need not remain an active catalog entry

`.lf/flows/live-loopflow-demo.yaml` and `.lf/skills/live-loopflow/{review,write}.md`
are staged acceptance machinery whose artifact paths are all in scratch. The
retained final-review note reports that invocation accepted and finished.
Delete these launchable fixtures from the repository when preserving the final
evidence in PR notes/durable documentation. Keep `.lf/directions/task-continuation.md`:
it encodes useful regression requirements rather than a second runtime.

### 6. Wave interpreter removal is adjacent, separately scoped work

The previous inventory in `wave-playhead-removal.md` still names executable
root wrapping, queue/return stack, hidden __flow-step, resident protocol and
Swift projections. It is not the missing Task first/finally deletion. Preserve
its journal/client migration boundary; do not make this review an implicit
authorization to delete it.

`FlowPosition` still imports QueuedInvocation and StepRef from the Wave playhead
module. That ownership should move with the separately scoped deletion, without
copying its types or changing captured data. One storage-backed Task driver and
one ordinary adapter are not themselves evidence of two navigation algorithms.

## What the diff already eliminated, and what must stay

Source searches found no current Task kickoff/iterate/gate phase selector,
phase cursor, or first/loop/finally dispatcher. Historical SQL and populated
migration fixtures still mention those names; retain them as upgrade evidence.
The base branch already had finite Task Flow positions. This branch restores
repetition by extending Flow topology, rather than deleting another live phase
machine. It removes Task-only XOR refusal and shares cursor settlement.

Keep exact claims, Run success evidence, human authority, captured definitions,
legacy verdict decoding, PR merge/rotation facts, Ask completion, and interrupted
operation evidence. None becomes redundant merely because phases disappear.
The new boundary-policy and selected-XOR-body duplication was already removed
in prior compression; do not claim those deletions again.

## Accepted direction and next proof

The human confirmed explicit schema edges, rejected the human gate concept,
and placed both double-loop backward targets at implement. Human review finishes
any design clarification before returning to implementation.
Do not ask for those choices again. Resolve how steps involve a human: ordinary
steps using Ask, explicitly interactive steps, or both. Then trace the chosen
interaction through the separate human-boundary machinery listed above before
settling the deletion boundary. The prior plan merely conditionalizing gate
buttons was insufficient. Declared-target parity, retained interaction history
and exact decision recovery remain required proof.

This pass performed source/caller/diff inspection only. No runtime tests,
provider launch, Task/Session mutation, installation, commit or publication was
performed. Previously recorded test and live-demo passes are retained evidence,
not rerun or enlarged into a full acceptance claim. The inventory is bounded by
the inspected repository; external catalog callers and historical stored-state
normalization still require proof before deletion.
