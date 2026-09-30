---
requires: a reviewable change and publication authority
produces: published or updated PR
---
Publish the current change when the caller's publication boundary is satisfied.

Inspect the diff against its actual base, including uncommitted work, the
accepted outcome and verification. A completed implementation or reconciliation
step does not establish that required behavior works. Resolve clear gaps;
return a concrete blocker when the publication criteria do not hold.

Run `lf task pr publish`. The command owns pushing and PR creation/update, consumes
valid prepared copy, and otherwise generates copy through its pr-message
template. It marks the PR ready and returns the URL without opening a browser.
Use explicit title/body overrides only when the caller supplied or revised them.
Inspect the returned PR for scope, supported claims and material proof limits;
correct its copy through the same operation when necessary.

Keep publication separate from merge. This skill neither arms auto-merge nor
completes the Task. Open the review page with `lf task pr open` only when requested.
Preserve unrelated active contributions rather than staging them as this change.
