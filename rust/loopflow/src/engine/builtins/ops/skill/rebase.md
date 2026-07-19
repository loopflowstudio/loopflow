---
produces: rebased branch (or no-op if up-to-date)
---
Rebase this branch onto main, resolving conflicts.

## Orientation

Before starting, orient yourself in this branch:

- Read `scratch/` — design docs and notes for the current work live here
  (`scratch/<branch>.md` is this PR's design; `scratch/questions.md` holds open
  questions and assumptions).
- Read wave/PM context only when the seed names the exact wave, task, project,
  or a concrete coordination question; never infer it or repair access as a
  prerequisite.
- Read the repo's agent doc (`CLAUDE.md` / `AGENTS.md`) for conventions.

Write design artifacts, notes, and open questions under `scratch/`. Don't
re-derive what these already record.

## Goal

Resolve the existing Loopflow-owned rebase. The waiting parent verifies and
pushes after this agent exits.

## Workflow

### 1. Understand the conflict
```bash
git status --short
```
Read `<lf:rebase-conflict>` for the pinned target and affected paths. The
sequencer already exists. Do not fetch, start another rebase, delegate to a
subagent, or run raw `git rebase` lifecycle commands.

### 2. Resolve and continue

If conflicts occur:

```bash
# See which files have conflicts
git status

# After resolving the current conflict
lf rebase --continue
```

**Conflict resolution strategy:**

- **Files central to the branch's intent:** Preserve the branch's changes named by the conflict context and surrounding code.
- **Files outside the branch's scope:** Accept main's version. The branch probably touched these incidentally.
- **Both versions are valid:** Combine manually if both changes make sense.
- **Ambiguous or high-risk conflicts:** Do not guess. In interactive runs, ask the user. In headless runs, write the ambiguity and options to `scratch/questions.md` and stop.

`lf rebase --continue` stages the resolved conflict paths and checks that this
agent owns the operation. Repeat until it reports completion. Loopflow records
the reviewed resolution for later identical conflicts; rerere auto-staging stays
disabled, so unrelated paths are never staged with it.

### 3. Verify the resolution

Run the smallest behavioral test that exercises the reconciled behavior, once,
after the rebase completes. Do not expand into the whole project suite or
unrelated lint/build checks here. Gate and CI own that broader proof.

Do not push. Exit after the focused proof; the waiting `lf rebase` process owns
Git postconditions and the single push.

## Abort

```bash
lf rebase --abort
```

Then:
- interactive: explain the failure and ask the user how to proceed
- headless: note what went wrong in `scratch/questions.md` and stop
