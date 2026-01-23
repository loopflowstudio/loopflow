---
produces: rebased branch (or no-op if up-to-date)
---
Rebase this branch onto main.

## Goal

Keep the branch current so merging is painless later. A clean rebase now prevents painful conflict resolution during landing. If conflicts are complex, abort and let the human decide—don't silently make the wrong choice about which code to keep.

## Workflow

### 1. Understand the branch's intent
```bash
git log main..HEAD --oneline
git diff main...HEAD --stat
```
Note which files this branch modified and what it's trying to accomplish.

### 2. Fetch and rebase
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
- **Both versions are valid:** Combine manually if both changes make sense. Otherwise, prefer the branch's version for modified files, main's version for everything else.

### 4. Verify and push
```bash
# Verify nothing broke
uv run pytest tests/

# Push the rebased branch
git push --force-with-lease
```

## If rebase goes wrong

```bash
# Abort and return to pre-rebase state
git rebase --abort
```

Then note what went wrong in `scratch/questions.md` and stop.

