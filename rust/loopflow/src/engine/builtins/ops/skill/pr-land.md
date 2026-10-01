---
requires: a reviewable change and authority to merge
produces: a merged PR or an actionable landing blocker
---
Land the current change through Loopflow's delivery operation.

Inspect the intended outcome, diff, proof and repository delivery requirements.
Fix readiness gaps and preserve useful design conclusions before scratch cleanup.
Keep unrelated active contributions out of this change.

Run `lf land` with the caller's requested disposition:

```bash
lf land                                # merge this PR; keep the Task open
lf land -c                             # complete the Task after merge
lf land --next <slug>                  # continue its serial PR chain
```

The operation owns preparation, sync, publication and exact-head auto-merge.
It returns after the request. It consumes valid prepared copy or generates it
through pr-message; use explicit title/body only for an intentional override.
Check the resulting PR's scope, claims and evidence limits.

If the caller requested a merged PR, observe that PR with `gh pr view --json
state,mergeCommit` and `lf pr checks`. Resolve failed checks with ci-fix, preserve
unrelated changes and renew the same landing disposition when the head changes.
Do not report success or Task completion until GitHub supplies merged evidence.
A pending request remains pending; polling does not acquire Flow authority.

A prepare/request/return endpoint finishes after `lf land` succeeds. Reviewer-owned
merge uses `lf submit`. Keep those endpoints distinct; invocation alone is not
proof of merge.
