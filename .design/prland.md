# Fix `lfpr land` to show PRs as merged

## What to build

Fix `lfpr land` to use `gh pr merge` instead of local squash-merge, so GitHub shows PRs as "merged" rather than "closed". Also consolidate the two implementations and improve error handling for common issues like dirty main branch.

## Root cause

The current `lfpr land` implementation in `src/loopflow/lfpr.py` does:
1. Fetch branch from origin
2. `git merge --squash` locally in main repo
3. `git push` to main
4. `git push origin --delete {branch}`

This causes GitHub to mark the PR as **closed** (not merged) because GitHub never sees a merge commit - it just sees the branch deleted after commits appeared on main.

The fix exists in `src/loopflow/cli/pr.py` which uses `gh pr merge --squash --delete-branch`. This properly marks PRs as merged. But `lfpr` (the actual CLI entry point) calls the broken implementation.

## Data structures

No new data structures needed. Existing types are sufficient.

## Key functions

```python
# In src/loopflow/lfpr.py, replace _land_pr with:

def _land_pr(add: bool, worktree: str | None, require_clean_design: bool) -> None:
    """PR-based landing using gh pr merge."""
    # ... validation and setup (keep existing) ...

    # NEW: use gh pr merge instead of local merge
    merge_cmd = ["gh", "pr", "merge", branch, "--squash", "--delete-branch", "--subject", title]
    if body:
        merge_cmd.extend(["--body", body])
    result = subprocess.run(merge_cmd, cwd=repo_root, capture_output=True, text=True)

    # ... sync main repo, cleanup worktree ...
```

The `cli/pr.py` implementation can be deleted since `lfpr.py` is the canonical one (it's what `lfpr` command invokes).

## Changes required

### 1. Fix `_land_pr` in `src/loopflow/lfpr.py`

Replace lines 410-443 (local merge) with `gh pr merge`:

```python
# BEFORE (broken):
subprocess.run(["git", "fetch", "origin", branch], cwd=main_repo, check=True)
result = subprocess.run(
    ["git", "merge", "--squash", f"origin/{branch}"],
    cwd=main_repo, ...
)
subprocess.run(["git", "commit", "-m", commit_msg], cwd=main_repo, check=True)
subprocess.run(["git", "push"], cwd=main_repo, check=True)

# AFTER (fixed):
merge_cmd = ["gh", "pr", "merge", branch, "--squash", "--delete-branch", "--subject", title]
if body:
    merge_cmd.extend(["--body", body])
result = subprocess.run(merge_cmd, cwd=repo_root, capture_output=True, text=True)
if result.returncode != 0:
    error_msg = result.stderr.strip() or result.stdout.strip() or "unknown error"
    typer.echo(f"Error: gh pr merge failed: {error_msg}", err=True)
    raise typer.Exit(1)
```

### 2. After merge, sync main repo

```python
# Fetch and fast-forward main repo to get merged changes
subprocess.run(["git", "fetch", "origin", base_branch], cwd=main_repo, check=True)
subprocess.run(["git", "checkout", base_branch], cwd=main_repo, check=True)
subprocess.run(["git", "pull", "--ff-only"], cwd=main_repo, check=True)
```

### 3. Remove the local branch delete

`--delete-branch` flag already deletes the remote branch. Just need to clean up local worktree/branch.

### 4. Delete duplicate code in `cli/pr.py`

The `land` command in `cli/pr.py` is dead code - `lfpr land` invokes `lfpr.py`, not `cli/pr.py`. Remove the duplicate to avoid confusion.

### 5. Improve error handling

The user mentioned "errors due to dirty main or other things". Current checks are reasonable but messages could be clearer. Add:

- Before merge: check main repo is clean and on correct branch
- Explain recovery steps in error messages
- Handle case where main is behind origin

## Constraints

- Must use `gh pr merge` - this is the only way GitHub marks PRs as merged
- The `--delete-branch` flag is essential - handles both remote and local branch deletion
- Must work from worktrees (the common case)
- Must pull merged changes into main repo so user has current state

## Done when

```bash
# Create test branch and PR
git checkout -b test-prland-fix
echo "test" >> /tmp/testfile && git add -A && git commit -m "test commit"
git push -u origin test-prland-fix
gh pr create --title "Test PR" --body "Testing land"

# Land should work and PR should show as merged
lfpr land

# Verify
gh pr view test-prland-fix --json state -q '.state'
# Expected: MERGED (not CLOSED)
```

Also verify:
- `lfpr land -a` handles uncommitted changes
- Landing from worktree removes worktree
- Landing from main repo removes local branch
- Error message is clear when main has uncommitted changes
