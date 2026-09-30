---
requires: a reviewable change and authority to merge
produces: a merged PR or an actionable landing blocker
---
Land the current change through Loopflow's delivery operation.

Inspect the intended outcome, diff, proof and repository delivery requirements.
Fix readiness gaps and preserve useful design conclusions before scratch cleanup.
Keep unrelated active contributions out of this change.

Run `lf pr land` with the caller's requested disposition:

```bash
lf pr land                 # merge this PR; keep the Task open
lf pr land -c              # complete the Task after merge
lf pr land --next <slug>   # continue its serial PR chain
```

The operation owns preparation, rebase, publication, exact-head auto-merge,
CI observation and repair. It consumes valid prepared copy or generates it
through pr-message; use explicit title/body only for an intentional override.
Check the resulting PR's scope, claims and evidence limits. Do not duplicate
its Git/GitHub mutations or launch another landing watcher.

If rebase or CI fails, use the named recovery path in this checkout and retry
with the same disposition. Rerunning land resumes a retained blocker without
inventing an empty commit. Preserve uncertainty about external effects and
require authoritative merge evidence before reporting success or completion.

Use `lf pr arm` when the requested endpoint is prepare/request/return rather
than watching to merge. Reviewer-owned merge uses `lf pr submit`. Keep those
endpoints distinct; invocation alone is not proof of merge.
