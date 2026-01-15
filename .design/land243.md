# Fix: land command should update local main after PR merge

## What to build

After `lfops land` merges a PR, the local main branch in the main repo must be updated to match origin/main, even when main is currently checked out.

## The problem

`_sync_main_repo` uses `git fetch origin main:main` to update local main without checkout. This fails when main is already checked out:

```
fatal: refusing to fetch into branch 'refs/heads/main' checked out at '/Users/jack/src/loopflow'
```

The workaround shown in the warning requires the user to manually fetch and pull:
```bash
cd /Users/jack/src/loopflow && git fetch origin && git checkout main && git pull
```

But that's unnecessary—if main is checked out, we can just `git pull` directly.

## Key function

```python
def _sync_main_repo(main_repo: Path, base_branch: str) -> bool:
    """Update local base_branch to match origin."""
    # Check if base_branch is currently checked out
    result = subprocess.run(
        ["git", "rev-parse", "--abbrev-ref", "HEAD"],
        cwd=main_repo,
        capture_output=True,
        text=True,
    )
    current_branch = result.stdout.strip() if result.returncode == 0 else ""

    if current_branch == base_branch:
        # Branch is checked out: fetch + reset to origin (fast-forward)
        subprocess.run(["git", "fetch", "origin", base_branch], cwd=main_repo, check=False)
        result = subprocess.run(
            ["git", "reset", "--hard", f"origin/{base_branch}"],
            cwd=main_repo,
            capture_output=True,
        )
        return result.returncode == 0
    else:
        # Branch not checked out: update ref directly
        result = subprocess.run(
            ["git", "fetch", "origin", f"{base_branch}:{base_branch}"],
            cwd=main_repo,
            capture_output=True,
        )
        return result.returncode == 0
```

## Constraints

- Must not change the checked-out branch in main repo (user may be working there)
- Must work whether main is checked out or not
- When main is checked out, use `fetch + reset --hard` (safe because we're syncing to origin, not losing work)
- The reset is safe because main should track origin/main exactly—any local-only commits would indicate a workflow problem

## Done when

```bash
# In a worktree
lfops land --create-pr

# Verify local main matches origin/main in main repo
cd /path/to/main-repo
git rev-parse main
git rev-parse origin/main
# Both should output the same SHA
```
