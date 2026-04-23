---
produces: rebased branch (or no-op if up-to-date)
---
Rebase this branch onto main, resolving conflicts.

## Goal

Recover from rebase conflicts and push a clean branch.

## Workflow

### 1. Understand the branch's intent
```bash
git log main..HEAD --oneline
git diff main...HEAD --stat
```
Note which files this branch modified and what it's trying to accomplish.

If `<lf:rebase-conflict>` is present, read it to understand what conflicted.

### 2. Rebase

Use the target from the conflict context if present. Default to `origin/main`:

```bash
git fetch origin main
git rebase origin/main
```

### 3. Handle conflicts

If conflicts occur:

```bash
# See which files have conflicts
git status

# After resolving each file
git add <file>
git rebase --continue
```

**Conflict resolution strategy:**

- **Files central to the branch's intent:** Preserve the branch's changes. These are the files listed in `git diff main...HEAD --stat`.
- **Files outside the branch's scope:** Accept main's version. The branch probably touched these incidentally.
- **Both versions are valid:** Combine manually if both changes make sense.
- **Ambiguous or high-risk conflicts:** Do not guess. In interactive runs, ask the user. In headless runs, write the ambiguity and options to `scratch/questions.md` and stop.

Repeat until `git rebase --continue` completes without conflicts.

### 4. Verify and push
```bash
# Run the project's test suite (see TESTING.md)
git push --force-with-lease
```

## Abort

```bash
# Abort and return to pre-rebase state
git rebase --abort
```

Then:
- interactive: explain the failure and ask the user how to proceed
- headless: note what went wrong in `scratch/questions.md` and stop
