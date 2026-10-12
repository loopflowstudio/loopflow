---
requires: a reviewable change
produces: recorded PR delivery with auto-merge requested, or verified merge when waiting
---
Land the current change through Loopflow's delivery operation.

Inspect the intended outcome, diff, proof and repository delivery requirements.
Fix readiness gaps and preserve useful design conclusions before scratch cleanup.
Keep unrelated active contributions out of this change. Preserve accepted later
checks and their exact unverified outcomes in durable PR copy before cleanup;
the Task conversation must be able to read them after merge without scratch.

```bash
lf land                 # prepare, request exact-head auto-merge, return
lf land --wait-and-fix  # wait for merge and repair failing CI
```

The operation owns preparation, sync, publication, and exact-head auto-merge.
It consumes valid prepared copy or generates it through pr-message; use explicit
title/body only for an intentional override. Check the resulting PR's scope,
claims and evidence limits. Do not duplicate its Git/GitHub mutations or launch
another landing watcher.

Bare land and `lf arm` return after recording intent. `lf pr reconcile` checks
recorded landings once; `lf ci watch` repairs actionable CI while installed and
running. `--wait-and-fix` observes and repairs its own PR every 15 seconds for at most
30 minutes, including with Desktop closed. It shares the incident reservation
with the watcher, so an unchanged incident receives one repair. Each check
releases the landing lock. Timeout leaves intent intact and returns held;
interruption returns stopped. Neither reports merge.
A successful handoff proves neither merge nor Task completion.

After verified merge, complete the Task when its accepted outcome is delivered.
File independently accepted later work as ordinary Tasks and link them with
`lf task comment`; a saved Task survives a failed linking comment, so retry only
that comment. No empty disposition or separate finishing Flow is required.
A Task has zero or one PR; further implementation belongs to another Task,
stacked on this one when dependent. Taskless delivery needs no synthetic Task.

If sync or CI fails, use the named recovery path in this checkout and retry.
Rerunning land retries a retained blocker without inventing an empty commit.
For an already-merged Task PR it succeeds immediately and reports whether Task
completion remains, preserving any finishing notes and edits.
Preserve uncertainty about external effects. Reviewer-owned merge uses
`lf submit`; report merge only from authoritative evidence, and completion only
from Task status.
