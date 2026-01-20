# Maestroux

Live git watching and stale worktree detection for Maestro, plus `lfops sync` and `lfops prune`.

## Status

Done. No remaining work identified in this branch.

## Follow-ups (Optional)

- Consider making the staleness threshold configurable (currently 14 days).

## Decisions

- Keep a two-layer refresh approach: git watcher for baseline updates, lfd socket for live sessions.
- Staleness uses four states: active, merged, remote deleted, inactive (days).
