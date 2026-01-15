# Robust Landing

## What to build

Make `lfops land` reliable by removing checkout dependencies and deleting `wtdoctor`.

## Problem

Landing fails with:
```
Error: failed to run git: fatal: 'main' is already used by worktree at '/Users/jack/src/loopflow'
```

This happens because `_sync_main_repo` tries to `git checkout main` in the main repo. With git worktrees, a branch can only be checked out in one place. If the main repo isn't on main (or if git thinks main is "locked"), checkout fails.

User quote: "we shouldnt crash because main is checked out, or the main worktree isnt on the main branch"

## What's been tried (last 24 hours)

**6ffa690** - First landing fix:
- Added `_sync_main_repo` that tries checkout, warns if it fails
- Added `_remove_worktree` that tries `wt` first, falls back to git
- Problem: still uses checkout, which fails

**c5e5b91** - Added resilience:
- Changed `_sync_main_repo` to return bool instead of raising
- Wrapped worktree removal in try/except
- Added the `recover` command
- Problem: still uses checkout approach

**630b988** - Added wtdoctor:
- Renamed `recover` to `wtdoctor`
- Added `git fetch origin main:main` in `_remove_worktree`
- Problem: `_sync_main_repo` STILL tries checkout before the fetch pattern

The fetch pattern (`git fetch origin main:main`) works in `_remove_worktree` but was never applied to `_sync_main_repo`.

**Root cause**: The error `"failed to run git: fatal: 'main' is already used..."` is from `gh pr merge --delete-branch`, not from loopflow code. The `gh` CLI:
1. Successfully merges the PR on GitHub
2. Tries to sync local main as part of `--delete-branch` cleanup
3. Fails the git checkout internally
4. Returns non-zero

We exit at line 1052 because gh returned non-zero, but the PR **was actually merged**. We never reach the merge verification at lines 1054-1067.

## Data structures

No new types. Simplifying existing flow.

## Key functions

```python
def _sync_main_repo(main_repo: Path, base_branch: str) -> bool:
    """Update local base_branch to match origin WITHOUT checkout."""
    # Use fetch with refspec to update local branch directly
    result = subprocess.run(
        ["git", "fetch", "origin", f"{base_branch}:{base_branch}"],
        cwd=main_repo,
        capture_output=True,
    )
    return result.returncode == 0
```

The fix: `git fetch origin main:main` updates the local `main` ref to match `origin/main` without requiring checkout. Works regardless of what branch is currently checked out.

## Changes

### 1. Don't use `--delete-branch` with gh (~5 lines)

`gh pr merge --delete-branch` tries to sync local main, which fails in worktrees. Handle remote deletion ourselves.

```python
# Current (broken):
merge_cmd = ["gh", "pr", "merge", branch, "--squash", "--delete-branch", "--subject", title]

# Fixed:
merge_cmd = ["gh", "pr", "merge", branch, "--squash", "--subject", title]
# ... after merge succeeds ...
subprocess.run(["git", "push", "origin", "--delete", branch], ...)  # delete remote
# _remove_worktree handles local worktree/branch
```

### 2. Fix `_sync_main_repo` (~10 lines)

Replace checkout-based sync with fetch-based sync:

```python
def _sync_main_repo(main_repo: Path, base_branch: str) -> bool:
    """Update local base_branch to match origin without checkout."""
    result = subprocess.run(
        ["git", "fetch", "origin", f"{base_branch}:{base_branch}"],
        cwd=main_repo,
        capture_output=True,
    )
    return result.returncode == 0
```

This is already used in `_remove_worktree`. Unify the pattern.

### 3. Delete `wtdoctor` (~250 lines)

Remove:
- `wtdoctor` command
- `_detect_issues` function
- `_fix_issue` function
- `Issue` dataclass
- All issue detection helpers

User quote: "checking worktrees for being same as main will not work for long as main keeps going and dead worktrees sit there. so i am skeptical wtdoctor is worth maintaining."

### 4. Update docs

Remove wtdoctor from command list. The `wt` tool handles worktree cleanup.

## Constraints

- **PR landing** (default): Must not require main branch to be checked out. Works when main repo is on any branch.
- **Local landing** (`--local`): Requires main to be checkout-able (no other worktree has it). This is a fundamental git limitation for local merges.
- Must print main_repo path so `cd $(lfops land)` works

## UI changes

None. Infrastructure-only: fixing CLI internals and removing a broken command.

## Done when

```bash
# From a worktree with a PR
uv run lfops land
# Lands successfully, prints main repo path

# Verify
cd /path/to/main/repo && git log -1 --oneline
# Shows the landed commit

uv run lfops wtdoctor
# "No such command"
```
