---
produces: rebased branch (or no-op if up-to-date)
fast-path: lf ops rebase
---
Rebase this branch onto main.

## Goal

Keep the branch current so merging is painless later. Most runs should finish in the fast-path. If the agent runs, recover from conflicts safely and push a clean branch.

## API

```bash
lf ops rebase [--onto BRANCH]
```

## Workflow

### 1. Understand the branch's intent
```bash
git log main..HEAD --oneline
git diff main...HEAD --stat
```
Note which files this branch modified and what it's trying to accomplish.

If `<lf:fast-path-failure>` is present, read it first to understand what already failed.

### 2. Rebase

Use the target from the failure context if present (`--onto BRANCH` or `Rebase onto: ...`). Default to `origin/main`:

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
