---
requires: Task design, implementation diff, measured replacement evidence, and executed proof
produces: one short slice review record, repaired bounded gaps, and next action/proof
default_agent: claude
action_style: procedural
---
Review the implemented slice through behavior, intent, and source. Repair
bounded gaps and return evidence; loop-decide owns navigation.

## Evidence first

Read the Task directive, `scratch/<branch>.md`, and the complete diff. Recover
the current slice, full target, forbidden outcomes, and Done when claims.
Separate observed results from expectations. Reuse applicable executed proof;
never treat an authored test as a pass. An unrunnable required proof is a stop:
record the exact failing command and blocker, then return it for resolution.

Demonstrate the important changed behavior through the real configured path
when available and safe. Otherwise run the closest local proof and label its
limits. Do not mutate production to manufacture evidence.

Inspect the model behind that behavior. Does each real-world concept map to one
representation? Has the change added a hop, duplicate owner, or fallback reader?
Would deleting code make the system more true? Follow normal and recovery paths
through their callers, writers, storage, and public interfaces.

A slice moves a real consumer end to end and deletes what it replaces in the
same cut. Adding an owner beside an existing one is not a slice. Search for the
replaced paths; passing behavior does not excuse a reachable competing owner,
dual write, or adapter preserving a caller that can migrate. Report measured
non-test lines added and removed, the compared revisions, and exclusions for
tests/generated files. Counts support the named replacement; they do not prove
it or impose a deletion quota on new capabilities.

Compare the current and previous implementation passes. Two consecutive passes
replacing nothing are a convergence blocker: name both passes and the next
consumer that needs to switch. Return that finding for resolution rather than
recommending another unchanged pass. The decision step owns navigation.

A worked example: one 30-minute implementation Run first failed its acceptance
test, moved interactive Sessions onto database rows end to end, and removed
16 predecessor items in the same cut: +396 / −409 non-test lines. Its record
named the switched reader, executed proof, and remaining gaps. Judge that
consumer replacement and honest boundary, not the duration or a deletion quota.

## Disposition

Fix clear, bounded gaps here and rerun the focused proof for changed behavior.
If the model requires a product decision or a larger change, return the exact
choice or next cut and proof. Preserve required behavior and recoverable data;
a smaller diff is not a reason to discard them. A passing slice does not satisfy
unproved whole-design claims.

Update one short review section in the existing slice record under `scratch/`.
Include date/scope, claims with pass or gap and executed evidence, measured
additions/deletions, removed paths, remaining findings, and the next action/proof.
Carry unresolved findings forward; replace superseded notes rather than adding
another review file. Return the record's path and a short takeaway. Do not
repeat the design or write a second conceptual review.

When all applicable Done when claims hold and the slice is coherent, publish
or refresh the Task PR with `lf pr publish`. Do not land or complete the Task.
