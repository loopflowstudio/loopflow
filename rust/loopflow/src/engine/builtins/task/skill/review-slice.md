---
interactive: false
requires: Task design, implementation diff, and runnable behavior
produces: repaired reviewable change | precise direction for the next slice
default_agent: claude
action_style: procedural
---
Review the implemented slice through behavior, intent, and source. Operate
independently; this is an autonomous loop step, not a Feedback boundary.

## Evidence first

Read the Task directive, `scratch/<branch>.md`, and the complete diff. Extract
the planned scope and every `Done when` claim before judging the implementation.
Keep observed results separate from expectations.

Demonstrate the most obvious and important user-changing behavior through the
real configured or production-like path available. Prefer a live environment,
real CLI/API surface, and observable logs or state. Never mutate production only
to manufacture proof. When the real boundary is unavailable or unsafe, use the
closest local proof and label the gap.

Build a compact evidence matrix:

| Claim | Planned behavior | Implemented behavior | Proof | Result |
|---|---|---|---|---|
| <Done when or material scope claim> | <design> | <observed implementation> | <command, live surface, test, source> | pass / gap |

Then inspect the source behind what was demonstrated. Review the implemented
shape against the design rather than narrating the diff. Look for missing scope,
behavior the design did not authorize, duplicate abstractions, unsafe operational
edges, and tests that prove wiring instead of value.

## Disposition

Fix clear, bounded gaps in this Task worktree and rerun their focused proof. If a
material change invalidates prior review, return precise direction for the next
slice rather than approving stale evidence.

When all applicable `Done when` claims hold and the slice is coherent, publish or
refresh the Task PR with `lf pr publish`. This review never lands or completes the
Task; the pinned final flow owns that boundary.
