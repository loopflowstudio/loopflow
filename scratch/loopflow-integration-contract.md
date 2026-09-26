# Current runtime integration contract

Integration update, 2026-09-25: the contribution assignments and the earlier
restriction against changing ConcreteStep below are historical. All bounded
contributions have finished. ConcreteXor now captures the router and every
branch, and Task FlowPosition uses the same ExecutionCursor as ordinary Flows.
Human and autonomous navigation both call cursor.finish. Exact route candidates
use `lf flow route PATH`; no shared route file remains. Backward edges have no
pass limit. Current proof and remaining recovery/release work are in
[cursor-integration.md](cursor-integration.md). The protocol below remains the
accepted direction where it does not describe those superseded contribution
boundaries.

The human chose the skill name loop-decide. One ordinary Flow with backward
edges runs standalone or with Task context; no new loopflow type or scheduler.
The shared reducer remains engine/transitions.rs::finish_step.

## Shared types

FlowDecision has Advance and Iterate only. FlowVerdict remains the persisted
struct { decision: FlowDecision, summary: String }. FlowProgress remains
{ repeats: BTreeMap<String,u32>, direction: Option<String>, verdict: Option<FlowVerdict> }.
Blocked belongs to execution outcomes, not FlowDecision. Keep FlowTransition
Next(usize), Repeat(usize), Finished, Blocked(String) internal names if useful.
Implicit forward progress within a pass retains direction until the deciding
occurrence advances explicitly. Counts stay per-edge across an invocation.
Old continue/complete and interim next/repeat serialized decisions must migrate.
Old blocked verdicts become recoverable blockers, never successful decisions.

## Ordinary engine contract

Engine/execution.rs owns generic traversal and uses the shared reducer. Keep the
existing executor trait, adding a default async checkpoint(&ExecutionCursor)
method for real persistence after EACH executed boundary, including nested XOR.
ExecutionCursor gains progress: transitions::FlowProgress and iteration: u32
(default old serialized cursor absence only where persisted compatibility needs it).
ExecutionContext retains UI progress and adds direction: Option<String>; no Copy.
SkillOutcome adds Decided(FlowVerdict) and Blocked(String); Completed and Waiting
remain. FlowOutcome adds Blocked(String). A missing required decision blocks.
The CLI executor owns durable invocation/active boundary/decision recording and
Ask handoff; the engine must not invent Task identities or do I/O itself.
When resuming with cursor.progress.verdict already saved, settle it without a
provider. Pending nested decisions must also recover without rerunning the router.
Pin a selected XOR body's ConcreteSteps in NestedCursor so recovery does not
reload a changed body. Do not change ConcreteStep definition in this contribution.

## Ownership

- task-transition agent: transitions.rs, durable.rs, store/sqlite/durable.rs,
  controller/task/mod.rs, pursue.yaml, canonical concept-review.md; its proof note.
- engine agent: engine/execution.rs and its proof note only.
- main: CLI flow execution/persistence/protocol, Ask integration, bin dispatch,
  LF command definitions, other docs/skills, remaining integration and proofs.

CLI seed for a claimed loop-decide step: lf flow decide advance|iterate "summary";
lf flow blocked "reason, attempted direction, and evidence" requests an Ask
running unblock, waits for human completion, returns the summary to the decision
agent to reassess. Main implements these adapters. Failed Ask launch stays
recoverable. Human gates retain their exact separate authority.
