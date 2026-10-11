---
requires: a reviewable change
produces: a ready PR assigned to its reviewer
---
Prepare the current change for the reviewer's merge click.

Inspect the intended outcome, diff, proof and repository delivery requirements.
Preserve unrelated active contributions. Fix clear readiness gaps and retain
useful design conclusions and accepted later checks in durable PR copy before
scratch cleanup.

Run `lf pr submit --create-pr`. It stages the change, syncs, clears scratch,
creates or updates the PR, marks it ready and assigns the reviewer. It consumes
valid prepared copy or generates it through pr-message. Supply explicit copy
only for an intentional override, then check the returned scope and proof claims.

After verified merge, the Task's landing Flow or next
Task/Wave operation files linked follow-ups or records none needed, then completes
it. A Task has at most one PR; additional delivery belongs to another Task.
Repair a reported conflict in this checkout using the existing sync operation,
then retry. Preserve actual errors and uncertain external effects.

Return the PR and any unresolved blocker. Stop at the manual-merge handoff;
do not enable auto-merge, merge, or advance a Task's Flow on the reviewer's behalf.
