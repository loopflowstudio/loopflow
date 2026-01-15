# Branch Cleanup

**What to build:** Make `lfops land` cleanup resilient to mid-flight failures.

## Problem

`lfops land` can fail after the merge succeeds but before cleanup completes, leaving:
- Remote main merged ✓
- Local main stale ✗
- Worktree/branch in indeterminate state ✗

Failure points after "point of no return":

**`_land_pr`** (after `gh pr merge` at line 1019):
- `_sync_main_repo` can fail at `git fetch` (network), `git checkout` (dirty main), or `git pull --ff-only` (local commits)
- `_remove_worktree` can fail if worktree has uncommitted changes

**`_land_local`** (after `git push` at line 1206):
- `_remove_worktree` can fail similarly

## Solution

After the point of no return, cleanup operations should be best-effort with clear recovery guidance.

### Changes to `_land_pr`

```python
# After gh pr merge succeeds, cleanup is best-effort
try:
    _sync_main_repo(main_repo, base_branch)
except subprocess.CalledProcessError:
    typer.echo(f"Warning: Could not sync {base_branch}. Run manually:", err=True)
    typer.echo(f"  cd {main_repo} && git fetch origin && git checkout {base_branch} && git pull", err=True)

# Worktree cleanup is also best-effort
was_in_worktree = repo_root != main_repo
if was_in_worktree:
    try:
        _remove_worktree(main_repo, branch, repo_root)
    except Exception:
        typer.echo(f"Warning: Could not remove worktree. Run manually:", err=True)
        typer.echo(f"  wt remove {branch}", err=True)
else:
    subprocess.run(["git", "branch", "-D", branch], cwd=main_repo, capture_output=True)

typer.echo(f"Landed {branch} onto {base_branch}.")
```

### Changes to `_sync_main_repo`

Make it not use `check=True` so callers can handle failures:

```python
def _sync_main_repo(main_repo: Path, base_branch: str) -> bool:
    """Sync main repo to latest. Returns True if successful."""
    # ... fetch, checkout, pull without check=True
    # Return False on failure instead of raising
```

### Changes to `_land_local`

Same pattern - wrap cleanup in try/except after push succeeds.

## One-time cleanup (done)

```bash
# Deleted 13 stale local branches
git branch -D agentstab background cp fuck gemini laozi liveupdate \
  maestro newrepos onboarding uiexplorations voices worktree-ux

# Pruned stale remote tracking refs
git remote prune origin

# Deleted 2 actual remote branches
git push origin --delete release-v0.5.2 uiexplorations
```

## Done when

```bash
# Simulate failure scenario - land should complete with warnings, not crash
# 1. Create test branch and PR
# 2. Dirty main repo with uncommitted changes
# 3. Run lfops land
# 4. Should see "Warning: Could not sync main" but still report success
# 5. Manual cleanup instructions should be accurate
```
