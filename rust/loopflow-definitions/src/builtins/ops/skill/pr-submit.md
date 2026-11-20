---
requires: a reviewable change
produces: a ready PR prepared for manual merge
---
Prepare the current change for the reviewer's merge click.

Check the outcome wanted, the diff and what was run against it, and fix clear
readiness gaps. Submitting clears the files this branch added under
`scratch/`, so first move into the PR copy what the code can't tell a later
reader: what people said that shaped it, word for word, what a run showed, and
checks that can only run after merge. Leave unrelated work out.

Run `lf pr submit`. It stages, syncs, clears added scratch files, creates or
updates the PR and marks it ready, keeping reviewers and assignees. It writes
the PR copy itself; pass a title or body only to override on purpose, then
read the PR for claims the work doesn't back.

When reviewers are named, run `lf pr request-review -r <login>` (or
`-r <org/team>`); don't infer reviewers from assignees. `lf pr reviews` shows
requests and submitted reviews.

On a conflict, repair it here with `lf sync` and retry. Report real errors, and
say so when unsure whether an external step happened.

Return the PR and any blocker. Stop at the merge click: don't enable
auto-merge, merge, or advance a Task's Graph.
