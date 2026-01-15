# landing: Fix lfops land for worktree workflows

## What to build

Fix `lfops land` to work reliably with full sync (checkout main + pull), and clear `.design/` contents before merging to main.

## The problem

Three issues keep recurring:

1. **"main is already used by worktree" error** - The error comes from either:
   - `git checkout main` in main_repo when main is checked out elsewhere
   - `wt remove` trying to checkout main as part of its cleanup

2. **PRs closed but not merged** - Previous fixes that bypassed checkout issues accidentally used local git operations instead of GitHub's merge API.

3. **`.design/` contents end up on main** - Currently cleared after merge. Should be cleared before merge so they never touch main.

## Root cause

The current flow has ordering and robustness issues:

```python
# Current flow after gh pr merge:
git checkout main     # May fail if main checked out elsewhere
git pull --ff-only
wt remove branch      # May fail with same checkout issue
clear_design_artifacts()  # Too late - already on main
```

The error message format "Error: failed to run git:" suggests the failure is in `wt remove`, not our direct git calls. worktrunk may be trying to checkout main as part of removing the current worktree.

## Key changes

### 1. Clear .design BEFORE merge

```python
def _clear_design_and_push(repo_root: Path) -> bool:
    """Delete .design/* contents, commit, push. Returns True if changes made."""
    design_dir = repo_root / ".design"
    if not design_dir.exists():
        return False

    files = list(design_dir.glob("*"))
    if not files:
        return False

    for f in files:
        f.unlink() if f.is_file() else shutil.rmtree(f)

    subprocess.run(["git", "add", "-A", str(design_dir)], cwd=repo_root, check=True)
    subprocess.run(["git", "commit", "-m", "clear .design/"], cwd=repo_root, check=True)
    subprocess.run(["git", "push"], cwd=repo_root, check=True)
    return True
```

### 2. Smart sync: skip checkout if already on main

```python
def _sync_main_repo(main_repo: Path, base_branch: str) -> None:
    """Sync main repo to latest. Handles checkout gracefully."""
    # Get current branch in main repo
    result = subprocess.run(
        ["git", "rev-parse", "--abbrev-ref", "HEAD"],
        cwd=main_repo, capture_output=True, text=True
    )
    current = result.stdout.strip()

    subprocess.run(["git", "fetch", "origin", base_branch], cwd=main_repo, check=True)

    if current == base_branch:
        # Already on main, just pull
        subprocess.run(["git", "pull", "--ff-only"], cwd=main_repo, check=True)
    else:
        # Try checkout - may fail if main checked out elsewhere
        result = subprocess.run(
            ["git", "checkout", base_branch],
            cwd=main_repo, capture_output=True, text=True
        )
        if result.returncode != 0:
            typer.echo(f"Note: Could not checkout {base_branch} in main repo")
            typer.echo(f"  {result.stderr.strip()}")
            typer.echo(f"Run 'cd {main_repo} && git checkout {base_branch}' manually")
            return  # Continue with cleanup, don't fail
        subprocess.run(["git", "pull", "--ff-only"], cwd=main_repo, check=True)
```

### 3. Remove worktree without wt

Use git directly to avoid any wt-specific checkout behavior:

```python
def _remove_worktree(main_repo: Path, branch: str, worktree_path: Path) -> None:
    """Remove worktree and branch using git directly."""
    # Force remove worktree (handles deleted/moved directories)
    subprocess.run(
        ["git", "worktree", "remove", "--force", str(worktree_path)],
        cwd=main_repo, capture_output=True
    )

    # Delete local branch
    subprocess.run(
        ["git", "branch", "-D", branch],
        cwd=main_repo, capture_output=True
    )
```

## Revised flow

```python
def _land_pr(...):
    # 1. Handle uncommitted changes (existing)
    # 2. Push if needed (existing)

    # 3. NEW: Clear .design before merge
    if _clear_design_and_push(repo_root):
        typer.echo("Cleared .design/")

    # 4. Merge via GitHub (existing)
    gh pr merge --squash --delete-branch

    # 5. Verify MERGED state (existing)

    # 6. NEW: Smart sync main repo
    _sync_main_repo(main_repo, base_branch)

    # 7. NEW: Remove worktree directly
    if was_in_worktree:
        _remove_worktree(main_repo, branch, repo_root)

    # 8. Print main repo path
```

## Constraints

- Must use `gh pr merge` to ensure PR shows as "merged"
- Must clear `.design/` BEFORE merge (commit to feature branch, then merge)
- Checkout failure should warn, not error (user can sync manually)
- Use `git worktree remove` instead of `wt remove` to avoid any wt-specific behaviors

## Apply to both modes

Same fixes needed for `_land_local`:
- Clear .design before squash-merge commit
- Use smart sync for main repo
- Use git worktree remove directly

## Done when

```bash
# Test both scenarios:

# 1. Main repo has 'main' checked out
cd /path/to/repo && git checkout main
cd /path/to/repo.feature
lfops land  # Should work

# 2. Main repo has different branch checked out
cd /path/to/repo && git checkout other-branch
cd /path/to/repo.feature
lfops land  # Should work (with warning about checkout)

# Verify for both:
# - .design/ cleared BEFORE merge
# - PR shows as "Merged" on GitHub (PR mode only)
# - No "main is already used" error
# - Worktree removed
```
