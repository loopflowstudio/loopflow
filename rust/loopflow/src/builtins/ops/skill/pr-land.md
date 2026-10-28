---
requires: a reviewable change
produces: recorded PR delivery with auto-merge requested, or verified merge when waiting
---
Land the current change through Loopflow's delivery operation.

Inspect the intended outcome, diff, proof and repository delivery requirements.
Fix readiness gaps and preserve useful design conclusions before any configured
scratch cleanup. Cleanup removes committed additions while preserving existing scratch files.
Keep unrelated active contributions out of this change. Preserve accepted later
checks and their exact unverified outcomes in durable PR copy before cleanup;
They must remain readable after merge without scratch.

```bash
lf land                 # prepare, request exact-head auto-merge, return
lf land --wait-and-fix  # wait for merge and repair failing CI
```

The operation checkpoints the change, clears added scratch files, syncs, publishes,
and requests exact-head auto-merge. Existing scratch files and edits remain.
It consumes valid prepared copy or generates it through pr-message; use explicit
title/body only for an intentional override. Check the resulting PR's scope,
claims and evidence limits. Do not duplicate its Git/GitHub mutations or launch
another landing watcher.

Bare land and `lf arm` return after recording intent. `lf pr reconcile` checks
recorded landings once and repairs nothing. `--wait-and-fix` observes and
repairs its own PR every 15 seconds until verified merge, closure, a concrete
blocker or interruption, including with Desktop closed. An unchanged incident
receives one repair. Each check
releases the landing lock. Elapsed time never stops waiting or triggers a rerun.
Interruption retains intent and returns stopped; a concrete blocker returns held.
Neither reports merge.
A successful handoff proves neither merge nor Task completion.

For an associated Task, verified merge leaves an open Task available for optional follow-up work. Register
new work through ordinary Task creation when useful, then `lf task complete` when
the outcome is satisfied. No follow-up disposition is required. `ship` runs gate
and waited landing; the Task graph's `end` requests completion. Recommend follow-up
Tasks for distinct outcomes. Continued work in the same Task can publish another
PR; earlier PRs remain history. No transition command is required.
Taskless delivery needs no synthetic Task.

If sync or CI fails, use the named recovery path in this checkout and retry.
Rerunning land retries a retained blocker without inventing an empty commit.
For an already-merged Task PR it succeeds immediately and reports whether Task
completion remains, preserving any finishing notes and edits.
Preserve uncertainty about external effects. Reviewer-owned merge uses
`lf submit`; report merge only from authoritative evidence, and completion only
from Task status.
