# wtdoctor: detect and fix worktree inconsistencies

## Summary

Added `lfops wtdoctor` command to detect and fix repository inconsistencies, with specific support for cleaning up squash-merged worktrees.

## Implementation

### New detection: squash-merged worktrees

Worktrees whose content matches `origin/main` indicate work that was squash-merged via PR. Git doesn't consider these "merged" because the commits differ, but the content is identical. The new detection compares branch content against origin/main using two-dot diff (`git diff origin/main..branch --stat`).

### Enhanced worktree removal

`_remove_worktree()` now accepts a `base_branch` parameter and fetches `origin/{base_branch}:{base_branch}` before calling `wt remove`. This ensures `wt` has up-to-date main branch info to correctly detect squash-merged branches.

### Command rename

Renamed from `recover` to `wtdoctor` to better describe its purpose as a diagnostic/fix tool for worktree issues.

## Issue types detected

| Type | Description | Fix |
|------|-------------|-----|
| `stale_main` | Local main behind origin | checkout + pull |
| `orphan_branch` | Branch with no remote/worktree | delete branch |
| `orphan_worktree` | Stale .git/worktrees entries | git worktree prune |
| `stale_ref` | Deleted remote branches still tracked | git remote prune |
| `squash_merged` | Worktree content matches main | remove worktree + branch |
