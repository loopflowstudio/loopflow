---
requires: a reviewable change
produces: published or updated PR
---
Publish the current change as a ready PR once it meets the caller's bar for publishing.

Read the diff against its actual base, uncommitted work included, next to the
outcome wanted and what was checked. A finished implementation step doesn't
mean the behavior works. Fix clear gaps; if the change isn't ready, return the
concrete blocker.

Run `lf sync` and resolve conflicts in place: they are routine work, never a
question, and a published PR merges cleanly with its base. Then run
`lf pr publish`. It pushes, creates or updates the PR, writes its copy, marks
it ready and prints the URL. Pass a title or body only when one was supplied.
Read the PR for claims the work doesn't back, and fix its copy with the same
command.

Publishing is not merging: don't arm auto-merge or complete the Task. Leave
unrelated work out.

In a persistent document workspace, commit the documents first with
`lf commit -m "<change>" <path>...`. That leaves other staged and unstaged work
alone and keeps `scratch/` local.
