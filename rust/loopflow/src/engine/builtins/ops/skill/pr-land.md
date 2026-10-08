---
requires: a reviewable change
produces: recorded PR delivery with auto-merge requested, or verified merge when waiting
---
Land the current change through Loopflow's delivery operation.

Inspect the intended outcome, diff, proof and repository delivery requirements.
Fix readiness gaps and preserve useful design conclusions before scratch cleanup.
Keep unrelated active contributions out of this change. Preserve accepted later
checks and their exact unverified outcomes in durable PR copy before cleanup;
follow-through must be able to read them after merge without scratch.

```bash
lf land                 # prepare, request exact-head auto-merge, return
lf land --wait          # also wait for authoritative merge
```

The operation owns preparation, sync, publication, and exact-head auto-merge.
It consumes valid prepared copy or generates it through pr-message; use explicit
title/body only for an intentional override. Check the resulting PR's scope,
claims and evidence limits. Do not duplicate its Git/GitHub mutations or launch
another landing watcher.

Bare land and `lf arm` return after recording intent. `lf pr reconcile` checks
recorded landings once; `lf ci watch` repairs actionable CI while installed and
running. An unchanged incident receives one repair. `--wait` observes every
15 seconds for at most 30 minutes, releasing the landing lock between checks.
Timeout or interruption leaves intent intact and reports held, not merged.
A successful handoff proves neither merge nor Task completion.

For a Task, verified merge leaves follow-through pending. The `ship` Flow runs
follow-through after waited landing: file accepted remaining obligations as linked
Tasks or record none needed, then call `lf task complete`. If working standalone
and merge is verified, read `lf help follow-through` and perform that remaining
step. Bare delivery hands pending finishing work to the next Task/Wave operation;
no new watcher or automatic conversation wake is created. Taskless delivery
requires no synthetic Task or follow-up. Additional PR work needs another Task,
stacked on this one when dependent; a Task never rotates to a second PR.

If sync or CI fails, use the named recovery path in this checkout and retry.
Rerunning land resumes a retained blocker without inventing an empty commit.
Preserve uncertainty about external effects. Reviewer-owned merge uses
`lf submit`; report merge only from authoritative evidence, and completion only
from Task status after the follow-through disposition is durable.
