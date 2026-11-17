---
produces: synced branch (or no-op if up-to-date)
---
Resolve the conflicts in the merge already in progress, then let the sync finish.

`<lf:sync-conflict>` names the target and the conflicted paths. The merge is
already under way: don't fetch, start another sync, hand it to a subagent or
run raw `git merge`. Read the branch's plan in `scratch/` to know what this
branch is for.

```bash
git status                # conflicted files
lf sync --continue        # after resolving; stages the resolved paths
lf sync --abort           # give up and go back
```

- **Files central to the branch:** keep the branch's changes, read against the
  incoming code around them.
- **Files the branch touched in passing:** take the target's version.
- **Both sides valid:** combine them by hand.
- **Unclear and risky:** ask when someone is here. Headless, explain the
  conflict and stop.

A merge started outside Loopflow continues with `lf sync --continue --adopt`.

When the sync completes, run the smallest test that exercises the combined
behavior, once. Suites belong to gate and CI. Don't push; the waiting `lf sync`
does the one push.
