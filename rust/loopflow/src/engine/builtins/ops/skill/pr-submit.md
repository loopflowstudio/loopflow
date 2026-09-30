---
requires: a reviewable change and manual-merge handoff authority
produces: a ready PR assigned to its reviewer
---
Prepare the current change for the reviewer's merge click.

Inspect the intended outcome, diff, proof and repository delivery requirements.
Preserve unrelated active contributions. Fix clear readiness gaps and retain
useful design conclusions in their durable owner before scratch cleanup.

Run `lf task pr submit --create-pr`. It stages the change, rebases, clears scratch,
creates or updates the PR, marks it ready and assigns the reviewer. It consumes
valid prepared copy or generates it through pr-message. Supply explicit copy
only for an intentional override, then check the returned scope and proof claims.

In a Task worktree, retain the requested disposition: bare submit keeps the
Task open; `-c` completes after merge; `--next <slug>` rotates its PR chain.
Repair a reported conflict in this checkout using the existing rebase operation,
then retry. Preserve actual errors and uncertain external effects.

Return the PR and any unresolved blocker. Stop at the manual-merge handoff;
do not enable auto-merge, merge, or advance a Task's Flow on the reviewer's behalf.
