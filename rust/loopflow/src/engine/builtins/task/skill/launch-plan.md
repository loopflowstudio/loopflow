---
requires: scratch/<branch>.md
produces: one core for this Task | live follow-up Tasks
action_style: procedural
---
Turn the design into an execution decision using the Task controls that already
exist. Do not create a manifest, receipt, marker, or new planning state.

## Orientation

Read the design, Task directive, current Work state, and repo guide. Consult
Wave and Project state only when the seed identifies them.

## Decide what stays here

Choose the ambitious single-threaded core whose implementation will settle the
contract for the rest of the work. Keep that core in this Task and describe its
boundary clearly enough for the following `implement` step. Avoid scaffolding:
the core should ship useful end-to-end behavior in this PR.

## Decide what becomes Tasks

For each remaining independently shippable outcome, choose one of two actions:

- If it can safely start against the current contract, create and launch it now
  with `lf task start <project> "<title>" --first slice --directive "<brief>"`.
  Use `--stack-on <current-task>` when it must build on this Task's branch.
- If it depends on decisions this core has not settled, leave it in the design
  as a named follow-up. Create it after this PR settles; do not invent durable
  staging state inside Loopflow.

Task directives must stand alone after `scratch/` is cleared: include intent,
constraints, and done-when proof. `--first slice` is the existing flow-level
way to skip another kickoff design conversation for work already designed
here; ordinary Tasks keep the default human kickoff gate.

Finish with a short accounting of the core retained here, Tasks launched, and
follow-ups intentionally deferred.
