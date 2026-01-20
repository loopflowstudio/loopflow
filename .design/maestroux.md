# Maestroux

Live git watching and stale worktree detection for Maestro, plus `lfops sync` and `lfops prune`.

## Status

Done. No remaining work identified in this branch.

## Implementation

- Native git watcher refreshes worktrees on worktrees/refs/index changes (no lfd required).
- Auto-sync timer runs `lfops sync` every 120s and refreshes worktrees.
- Staleness detection runs after refresh and auto-prunes merged or remote-deleted clean worktrees.
- Sidebar shows lfd connection status; worktrees show staleness badges.

## Follow-ups (Optional)

- Consider making the staleness threshold configurable (currently 14 days).

## Decisions

- Keep a two-layer refresh approach: git watcher for baseline updates, lfd socket for live sessions.
- Staleness uses four states: active, merged, remote deleted, inactive (days).
