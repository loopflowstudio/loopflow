# Branches

Make `lfops land` resilient to mid-flight failures and add a `recover` command.

## Implementation

Two features in one branch:

**Resilient land:** `_land_pr` and `_land_local` catch cleanup failures and print manual recovery instructions instead of crashing. The merge itself is the critical operation; worktree/branch cleanup is best-effort.

**`lfops recover`:** Dry-run by default, `--fix` to apply. Detects:
- Stale default branch (local behind origin)
- Orphan branches (merged, no remote, no worktree)
- Orphan worktrees (stale entries in .git/worktrees)
- Stale remote refs

The orphan branch detection is conservative—only flags branches that are merged AND have no remote AND have no worktree. Unmerged work is never auto-deleted.

Both `_detect_issues` and `_fix_issue` dynamically determine the default branch via `git symbolic-ref` rather than assuming "main".
