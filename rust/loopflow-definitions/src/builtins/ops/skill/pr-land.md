---
requires: a reviewable change
produces: PR with auto-merge requested, or merged when waiting
---
Land the current change with `lf land`.

Check the intended outcome, the diff, the runs and checks behind it, and the
repository's delivery requirements. Fix readiness gaps first. Landing clears
files added under `scratch/`, so first move into the PR copy what the code
and git history can't tell a later reader: exact quoted direction, what a run
showed, and checks that can only run after merge with what they still need to show. Leave other people's unrelated work out of
this change.

```bash
lf land                 # prepare, request exact-head auto-merge, return
lf land --wait-and-fix  # wait for merge and repair failing CI
```

`lf land` commits, clears added scratch files (existing ones and their edits
stay), syncs, publishes, and requests exact-head auto-merge. It uses prepared
PR copy or writes it through pr-message; pass a title or body only to override
on purpose. Read the resulting PR for scope and claims. Don't repeat its Git or
GitHub steps or start a second landing watcher.

Bare `lf land` and `lf arm` return once the landing is recorded. `lf pr
reconcile` checks recorded landings once and repairs nothing.
`--wait-and-fix` checks its own PR every 15 seconds and repairs it until it
merges, closes, hits a concrete blocker, or is interrupted, with Desktop open
or not. An unchanged failure gets one repair. Each check releases the landing
lock. Elapsed time never stops the wait or triggers a rerun. Interrupted, it
keeps the landing and returns stopped; on a blocker it returns held. Neither
means merged, and a successful handoff means neither merge nor Task completion.

After merge, a Task stays open for follow-up. Create new Tasks for distinct
outcomes, and run `lf task complete` when the outcome is done; the Task graph's
`end` requests completion, and `ship` runs gate and a waited landing. More work
in the same Task can publish another PR; earlier PRs stay in its history.
Taskless delivery needs no Task.

If sync or CI fails, follow the named recovery in this checkout and retry.
Rerunning `lf land` retries a held blocker without an empty commit. On an
already-merged Task PR it succeeds at once and says whether Task completion is
still pending, keeping any finishing notes and edits. When unsure whether an
external effect happened, say so. For a reviewer's merge click, use `lf
submit`. Report merge only from GitHub, and completion only from `lf task
status`.
