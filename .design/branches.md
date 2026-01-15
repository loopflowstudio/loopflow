# Branches

Make `lfops land` resilient to mid-flight failures and add a `recover` command.

## Review

**Verdict:** Needs work

1. **`_detect_issues` hardcodes "main" instead of using default branch** (`lfops.py:1330`). The function correctly detects the default branch at line 1373 but then uses the literal `origin/main` at line 1330 when checking if local is stale. Should use `f"origin/{default_branch}"` after determining it.

2. **`_fix_issue` hardcodes "main"** (`lfops.py:1442`). Same issue—uses `git checkout main` instead of `git checkout {default_branch}`. The fix_cmd in the Issue is correct, but the actual implementation ignores it.

3. **Stale main detection runs from repo root, not main branch** (`lfops.py:1330`). The `HEAD..origin/main` comparison checks whatever branch you're on, not whether `main` is behind `origin/main`. Should be `git rev-list main..origin/main --count`.

## Design notes

The approach is sound: make post-merge cleanup best-effort with warnings. Two features in one branch:

- **Resilient land:** `_land_pr` and `_land_local` now catch cleanup failures and print manual recovery instructions instead of crashing.

- **`lfops recover`:** Dry-run by default, `--fix` to apply. Detects stale main, orphan branches, orphan worktrees, stale remote refs.

The orphan branch detection is conservative—only flags branches that are merged AND have no remote AND have no worktree. Unmerged work is never deleted without `--force` (not yet implemented per spec).
