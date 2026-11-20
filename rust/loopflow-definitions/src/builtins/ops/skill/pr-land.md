---
requires: a reviewable change
produces: PR with auto-merge requested, or merged when waiting
---
Land the current change with `lf land`.

Check the outcome wanted, the diff and what was run against it, and fix
readiness gaps first. Landing clears the files this branch added under
`scratch/`, so first move into the PR copy what the code and git history can't
tell a later reader: what people said, word for word, what a run showed, and
checks that can only run after merge. Leave unrelated work out.

```bash
lf land                 # prepare, request exact-head auto-merge, return
lf land --wait-and-fix  # stay until it merges, repairing failing CI
```

`lf land` commits, clears added scratch files, syncs, publishes and requests
auto-merge for the exact head. It writes the PR copy itself; pass a title or
body only to override on purpose, then read the PR for claims the work doesn't
back. Don't repeat its Git or GitHub steps or start a second watcher.

Bare `lf land` returns once the landing is recorded. `--wait-and-fix` watches
its own PR and repairs it until it merges, closes, hits a concrete blocker or
is interrupted. Interrupted, it returns stopped; on a blocker, held. Neither
means merged. Rerunning `lf land` retries a held blocker, and on an
already-merged PR it succeeds at once.

If sync or CI fails, follow the recovery it names in this checkout and retry.
Report a merge only from GitHub and Task completion only from
`lf task status`; merging doesn't complete a Task. For a reviewer's merge
click, use `lf submit` instead.
