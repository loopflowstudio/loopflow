---
requires: a reviewable change
produces: published or updated PR
---
Publish the current change as a ready PR once it meets the caller's bar for publishing.

Read the diff against its actual base, including uncommitted work, next to the
accepted outcome and what was checked. A finished implementation step doesn't
mean the behavior works. Fix clear gaps; if the change isn't ready, return the
concrete blocker.

Run `lf sync` first and resolve conflicts in place; a published PR merges
cleanly with its base. Conflicts are routine work, never a question. Then run
`lf pr publish`. It pushes, creates or updates the PR, uses prepared copy or
writes it through pr-message, marks the PR ready, and prints the URL without
opening a browser. Pass a title or body only when the caller supplied or
revised one. Read the PR for scope and claims the work doesn't back, and fix
its copy with the same command.

Publishing is not merging: don't arm auto-merge or complete the Task. Run `lf
pr open` only when asked. Leave unrelated work out of this change.

In a persistent document workspace, read the whole committed range and run
`lf commit -m "<accepted change>" <selected-path>...` before publishing. That
pushes the committed documents, leaves other staged and unstaged work alone,
and keeps `scratch/` local. Don't delete scratch or create an export worktree.
Later publishes carry new work past merged history on their own.
