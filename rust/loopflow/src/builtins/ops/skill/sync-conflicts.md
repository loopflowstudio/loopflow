---
produces: synced branch (or no-op if up-to-date)
---
Merge the target into this branch, resolving conflicts.

Read `scratch/` for this branch's design (`scratch/<branch>.md`) and open
questions (`scratch/questions.md`), and `AGENTS.md` for conventions. Read Wave
or Task context only when the seed names the exact wave, Task or Project.

## 1. Read the conflict

```bash
git status --short
```

`<lf:sync-conflict>` names the pinned target and the affected paths. The merge
is already in progress: don't fetch, start another sync, hand it to a
subagent, or run raw `git merge` commands.

A merge started outside Loopflow continues with `lf sync --continue --adopt`
(or `lf sync --abort --adopt`). A stopped Loopflow sync keeps its target and
continues with plain `--continue` or `--abort`. A sync whose caller is still
running needs that caller's recovery identity.

## 2. Resolve and continue

```bash
git status                # conflicted files
lf sync --continue        # after resolving the current conflict
```

- **Files central to the branch:** keep the branch's changes, guided by the
  conflict context and surrounding code.
- **Files outside the branch's scope:** take main's version; the branch
  probably touched them in passing.
- **Both sides valid:** combine them by hand.
- **Ambiguous or risky:** ask in an interactive conversation. Headless,
  explain the conflict and stop; the Wave operator reads the logs.

`lf sync --continue` stages the resolved paths and checks this agent runs the
sync. Loopflow remembers the resolution for identical later conflicts; rerere
auto-staging stays off, so unrelated paths never get staged with it.

## 3. Check the result

After the sync completes, run the smallest test that exercises the combined
behavior, once. Leave the full suite, static analysis and builds to gate and CI.

Don't push. Exit after that test; the waiting `lf sync` does the single push.

## Abort

```bash
lf sync --abort
```

Then:
- interactive: explain the failure and ask how to proceed
- headless: explain the exact blocker in ordinary output and stop
