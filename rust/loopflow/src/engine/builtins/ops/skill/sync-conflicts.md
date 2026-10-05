---
produces: synced branch (or no-op if up-to-date)
---
Merge the target into this branch, resolving conflicts.

## Orientation

Before starting, orient yourself in this branch:

- Read `scratch/` — design docs and notes for the current work live here
  (`scratch/<branch>.md` is this PR's design; `scratch/questions.md` holds open
  questions and assumptions).
- Read wave/PM context only when the seed names the exact wave, task, project,
  or a concrete coordination question; never infer it or repair access as a
  prerequisite.
- Read the repo's agent doc (`AGENTS.md`) for conventions.

Write design artifacts, notes, and open questions under `scratch/`. Don't
re-derive what these already record.

## Goal

Resolve the merge already in this checkout. When a sync caller is waiting, it
verifies and pushes after this agent exits.

## Workflow

### 1. Understand the conflict
```bash
git status --short
```
Read `<lf:sync-conflict>` for the pinned target and affected paths. The
merge state already exists. Do not fetch, start another sync, delegate to a
subagent, or run raw `git merge` lifecycle commands.

For a merge started by a supervisor without a Loopflow owner, use
`lf sync --continue --adopt` after resolving it (or `lf sync --abort --adopt`).
A stopped Loopflow sync retains its target and is adopted by ordinary
`--continue` or `--abort`. A live caller still requires its exact recovery identity.

### 2. Resolve and continue

If conflicts occur:

```bash
# See which files have conflicts
git status

# After resolving the current conflict
lf sync --continue
```

**Conflict resolution strategy:**

- **Files central to the branch's intent:** Preserve the branch's changes named by the conflict context and surrounding code.
- **Files outside the branch's scope:** Accept main's version. The branch probably touched these incidentally.
- **Both versions are valid:** Combine manually if both changes make sense.
- **Ambiguous or high-risk conflicts:** Ask in the current interactive conversation.
  Headless work explains the unresolved conflict and stops for the Wave operator
  to inspect its existing logs.

`lf sync --continue` stages the resolved conflict paths and checks that this
agent owns the operation. The merge resolves the combined branch changes in one
round. Loopflow records the reviewed resolution for later identical conflicts;
rerere auto-staging stays disabled, so unrelated paths are never staged with it.

### 3. Verify the resolution

Run the smallest behavioral test that exercises the reconciled behavior, once,
after the sync completes. Do not expand into the whole project suite or
unrelated static-analysis or build checks here. Gate and CI own that broader
proof.

Do not push. Exit after the focused proof; the waiting `lf sync` process owns
Git postconditions and the single push.

## Abort

```bash
lf sync --abort
```

Then:
- interactive: explain the failure and ask the present User how to proceed
- headless: explain the exact blocker in ordinary output and stop
