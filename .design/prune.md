# Prune

Two `lfops` commands for worktree maintenance: `lfops sync` fetches origin/main, `lfops prune` removes merged worktrees.

## Review

**Verdict:** Ready to ship

Clean implementation. The code follows STYLE.md conventions, tests cover the key logic paths, and documentation is updated.

## Design notes

**Squash merge detection:** PR state alone isn't sufficient for squash merges. The `is_merged` function checks both `pr_state == "merged"` AND whether the branch is an ancestor of `origin/main` via `git merge-base --is-ancestor`. This handles rebased/squash-merged branches correctly.

**Safety guards:** Never prunes main/master or dirty worktrees—checked explicitly before any removal.

**Sync before prune:** The `prune` command syncs main first so merge-base checks reflect the latest origin state.
