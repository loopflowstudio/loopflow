---
requires: a reviewable change
produces: recorded PR delivery with auto-merge requested
---
Land the current change through Loopflow's delivery operation.

Inspect the intended outcome, diff, proof and repository delivery requirements.
Fix readiness gaps and preserve useful design conclusions before scratch cleanup.
Keep unrelated active contributions out of this change.

Run `lf land` with the caller's requested disposition:

```bash
lf land                 # hand off this PR; keep the Task open
lf land -c              # complete the Task after merge
lf land --next <slug>   # continue its serial PR chain
```

The operation owns preparation, sync, publication, and exact-head auto-merge.
It records the settlement intent and returns. Later `lf pr reconcile` invocations
check recorded repository landings once and settle verified merges. `lf ci watch`
repairs actionable CI while it runs. An unchanged incident receives one repair;
unresolved evidence stays visible.
A successful handoff does not prove merge or Task completion. It consumes valid prepared copy or generates it
through pr-message; use explicit title/body only for an intentional override.
Check the resulting PR's scope, claims and evidence limits. Do not duplicate
its Git/GitHub mutations or launch another landing watcher.

If sync or CI fails, use the named recovery path in this checkout and retry
with the same disposition. Rerunning land resumes a retained blocker without
inventing an empty commit. Preserve uncertainty about external effects and
require authoritative merge evidence before reporting success or completion.

`lf arm` uses the same prepare/request/return operation. Reviewer-owned merge
uses `lf submit`. Neither arm nor land waits for CI or merge. Report the
handoff and any known blocker; report merge only from authoritative evidence.
