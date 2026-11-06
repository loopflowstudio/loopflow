---
requires: a reviewable change
produces: a ready PR prepared for manual merge
---
Prepare the current change for the reviewer's merge click.

Check the intended outcome, the diff, what was run and checked, and the
repository's delivery requirements. Fix clear readiness gaps. Submitting
clears files added under `scratch/`, so first move into the PR copy what the
code can't tell a later reader: what people said that shaped it, word for
word, what a run showed, and checks that can only run after merge. Leave unrelated work out.

Run `lf pr submit`. It stages the change, syncs, clears added scratch files,
creates or updates the PR and marks it ready, keeping reviewers and assignees.
It uses prepared copy or writes it through pr-message; pass a title or body
only to override on purpose, then read the PR's scope and claims.

When reviewers are named, run `lf pr request-review -r <login>` or `-r
<org/team>` on the PR. Don't infer reviewers from assignees. `lf pr reviews`
shows requests and submitted reviews; inline discussion stays on the PR page.
Publishing and submitting don't request reviews.

A Task's own graph completes it after merge. Suggest a follow-up Task for a
distinct outcome; more PRs from the same Task are fine. On a conflict, repair
it in this checkout with `lf sync`, then retry. Report real errors, and say so
when unsure whether an external step happened.

Return the PR and any blocker. Stop at the merge click: don't enable
auto-merge, merge, or advance a Task's Graph.
