# Loopflows: proposed usage and branch direction

Status: human-directed redesign, 2026-09-25. This replaces the earlier Task-only
continuation scope as the target of the Advance branch. Existing code and its
tests are evidence to reconcile, not acceptance of that implementation.

User direction:

> define backwards edges, decision protocols, and introduce the concept of a
> "loopflow": a flow with 1 or more loops/backwards edges, and then turn the
> first/loop/finally into a single loopflow that can be run the same way a flow
> is run

## Proposed usage documentation

Accepted clarification: "we can just accept loopflows anywhere we accept flows
basically." This is the compatibility contract, not an unresolved choice.

A **Flow** names a sequence of steps. A **loopflow** is a Flow with one or more
backward edges. Run it through the same command and execution path as any other
Flow. It needs no Task solely to repeat. Binding it to a Task supplies the Task's
context and existing work authority.

Anywhere a command, composed definition, or API accepts a Flow, it accepts a
loopflow. Keep the Flow type and its entry points. The term describes the edges
in the definition; it introduces no separate execution mode or setup requirement.
Ordinary execution constraints still apply, including exact human approval.

By default, finishing a step proceeds to the next step. One `loop-decide` skill owns
the decision step: Advance follows the forward edge; Iterate takes the declared
backward edge with direction for another pass. Work and review agents supply
evidence, not a decision file. A process exit is not a decision. A human gate
requires the exact human decision; an autonomous reviewer cannot supply it.

Blocked is a stopped execution outcome, not a third navigation choice. No
meaningful progress in the preceding pass is a reason to stop and ask for help;
new evidence or a narrowed hypothesis counts as progress. Reporting Blocked
opens one Ask, whose Session runs `unblock`. That skill uses `concept-review`
with the human by default, or works through a narrower question when the blocker
is specific. Completing the Ask returns its summary and artifact changes to the
decision agent for reassessment; it does not approve another Flow gate.

The former first/loop/finally arrangement becomes one definition:

```text
design → human design review → implement → compress → review-slice
                                  ↑                       ↓
                                  │                 concept-review
                                  │                       ↓
                                  └── Iterate ──────── loop-decide
                                                       ↓ Advance
                                                human demo → delivery

loop-decide reports Blocked → Ask Session running unblock → human Complete
                               (uses concept-review)          ↓
                                                    loop-decide reassesses
```

The initial steps occur once, the declared edge repeats the chosen section,
and the remaining steps run when its decision proceeds forward. "Finally"
means the chosen success path, not an unconditional cleanup handler. Delivery
uses the explicitly selected policy; continuation grants no merge permission.

Advance continues the saved Flow or accepts its exact human boundary. It is a
control over execution, not the source of the loop. A slice is a unit of work
inside the Flow; a pass is one traversal of a repeated section.

Illustrative invocation using the ordinary named-Flow surface:

```sh
lf feature
lf task run LOO-295 --flow feature
```

Both should interpret the same Flow definition and decisions. The latter adds
Task context. The standalone example is a target, not working behavior today.
Exact standalone persistence/resume commands still need design.

## Proposed skill guidance

Choose the Flow that describes the whole requested work. If it contains backward
edges, it is a loopflow; invoke it just as a Flow. Do not create a Task merely to
enable repetition. Follow the decision protocol supplied for the current step,
record evidence and any direction before settling it, and respect its allowed
targets and human authority. Continue only the saved invocation; completion
does not silently choose another Flow.

## Where the current implementation differs

- `engine/execution.rs::FlowEngine` rejects any repeat occurrence, including in
  entered xor paths. Its test explicitly enforces Task-only repetition. That
  test protects the previous design and must change for this goal.
- `controller/task/mod.rs` separately reduces repeat verdicts and drives the
  saved Task cursor. Ordinary Flow execution already has another cursor and
  executor. Removing only the generic refusal would skip decision semantics.
- The protocol is exposed as `lf task verdict` and requires a tracked Task and
  its current claimed Run. An unbound loopflow needs decision identity without
  inventing Task ownership.
- `RepeatPolicy` already represents a backward edge; `LoopReview` mixes its
  progress with a pending step decision. They offer reusable evidence, not a
  reason to keep the public model Task-specific.

## What must survive the simplification

One pinned definition, one execution cursor per invocation, fresh provider Runs
at autonomous boundaries, exact approval, stale-worker/decision rejection,
saved-result recovery without duplicate execution, honest blockers and limits,
and explicit delivery. Preserve old Work and recoverable execution facts during
migration; do not carry competing execution authorities forward indefinitely.

A loopflow is a property of a Flow definition, not a second schema,
scheduler, process type, or collection of first/loop/finally slots. The next
design must reconcile the existing generic and Task execution paths rather than
add a third. Do not replace persisted decisions with transient return values.

## Decisions to work through after the usage draft

1. Decision protocol: Advance/Iterate and a separate Blocked execution outcome
   are accepted. Connect the dedicated decision step to Ask exactly once and
   retain that Ask association through recovery. Do not reopen an unchanged,
   unresolved blocker automatically after the human completes the conversation.
2. Standalone invocation: where does its durable position live, how is it named,
   and how does Advance resume or open its human boundary without a Task?
3. Backward edges: define scope and budgets for multiple loops; decide whether
   nested/overlapping sections are necessary rather than inheriting the current
   autonomous-only validation as product policy.
4. Human Iterate at delivery: explicitly repeat the implementation section or
   select another authored edge; do not guess from the nearest autonomous step.

## Proof that would accept the branch

Run the same loopflow definition through ordinary Flow invocation and a bound
Task. In both, demonstrate the initial section once, a decision taking a backward
edge, a later decision reaching final steps once, and a visible stop for human
input or blocker. A stalled pass opens an Ask running unblock once; its human
completion resumes decision assessment without advancing a separate human gate.
Exercise at least two backward edges, exact/stale decisions,
interruption and saved-decision recovery. Compare their transition evidence;
Task binding must not change the meaning of the definition.

The existing Task-only fixture passes do not meet this proof. The current edits
add concept-review and documentation; they do not implement this redesign yet.

## Future direction

The human suggested loop-decide could eventually be a jev-agent. Preserve the
small decision contract so a future agent implementation can change independently
of Flow topology, persistence, and Ask behavior. This is a future direction, not
an additional implementation requirement for this branch.

## Current integration checkpoint (2026-09-25)

The earlier implementation-difference bullets above are the redesign baseline.
The shared reducer now runs ordinary and Task backward edges. `loop-decide`
owns typed decisions, Blocked connects to keyed Ask/unblock, and ordinary human
boundaries project as Flow Sessions. Design and delivery revision targets are
explicit backward edges. Ordinary invocation positions are saved under the
current Home and can be resumed. See `protocol-review.md` for current types,
repaired counterexamples, and the unresolved Wave/XOR/live-transport acceptance
gaps. These gaps remain part of the full design; this checkpoint is not shipment
or permission to narrow the accepted scope.
