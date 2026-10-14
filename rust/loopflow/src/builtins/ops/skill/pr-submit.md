---
requires: a reviewable change
produces: a ready PR prepared for manual merge
---
Prepare the current change for the reviewer's merge click.

Inspect the intended outcome, diff, proof and repository delivery requirements.
Preserve unrelated active contributions. Fix clear readiness gaps and retain
useful design conclusions and accepted later checks in durable PR copy before
scratch cleanup.

Run `lf pr submit`. It stages the change, syncs, clears committed scratch additions,
creates or updates the PR and marks it ready, preserving reviewers and assignees. It consumes
valid prepared copy or generates it through pr-message. Supply explicit copy
only for an intentional override, then check the returned scope and proof claims.

If the requested handoff names reviewers, use `lf pr request-review -r <login>`
or `-r <org/team>` on the existing PR. Do not infer reviewers from assignees.
Read `lf pr reviews` for requests and review submissions; inline discussion stays
on the PR page. Publishing and submitting do not themselves request reviews.

For an associated Task, leave completion to its selected work procedure after
verified merge. Recommend a follow-up Task for a distinct outcome; continued
publication in the same Task is supported and retains earlier PR history. Taskless delivery creates no Task or follow-up filing obligation.
Repair a reported conflict in this checkout using the existing sync operation,
then retry. Preserve actual errors and uncertain external effects.

Return the PR and any unresolved blocker. Stop at the manual-merge handoff;
do not enable auto-merge, merge, or advance a Task's Flow on the reviewer's behalf.
