# Maestroux

Live git watching and stale worktree detection for Maestro, plus `lfops sync` and `lfops prune` commands.

## Review

**Verdict:** Ready to ship

Clean implementation across four commits. No blocking issues.

## Implementation Summary

**GitWatcherService** (`Maestro/Maestro/Services/GitWatcherService.swift`)
- FSEvents-based watcher monitoring `.git/worktrees/`, `.git/refs/`, `.git/index`
- 500ms debounce via `pendingChanges` Set and Task sleep
- Handles worktrees-as-repos by resolving `.git` files to real git directories
- Actor-isolated for thread safety

**LFDEventService** (`Maestro/Maestro/Services/LFDEventService.swift`)
- Graceful connection handling: checks socket existence before connecting
- 5-second reconnect loop runs continuously
- Exposes `isConnected` via callback for UI indicator
- Changed `subscribe` to non-throwing (errors handled internally)

**Staleness detection** (`Maestro/Maestro/Services/WorktreeService.swift:212-248`)
- Four states: `.active`, `.merged`, `.remoteDeleted`, `.inactive(days:)`
- Checks: merged branches, remote ref existence, commit age (14 day threshold)
- Runs async after worktree refresh (doesn't block UI)

**CLI commands** (`src/loopflow/lfops/sync.py`, `src/loopflow/lfops/prune.py`)
- `lfops sync` — fetches origin/main, updates local ref if checked out
- `lfops prune` — finds merged worktrees, confirms, removes them
- Both integrate with existing `sync_main_repo` helper

**Python worktrees** (`src/loopflow/lf/worktrees.py:232-255`)
- `is_merged()` — checks PR state, then falls back to `merge-base --is-ancestor`
- `find_merged()` — filters worktree list to merged ones
- Handles squash merges correctly

**UI** (`Maestro/Maestro/Views/WorktreeSidebar.swift`)
- Connection indicator in header (green = lfd, gray = file watcher only)
- Stale worktrees show badges with icons and dimmed text
- Help text on hover explains each state

**Tests** (`tests/test_worktrees.py:125-218`)
- Coverage for `is_merged`: main/master exclusion, dirty exclusion, PR state, ancestor check
- Coverage for `find_merged`: filters correctly

## Design Notes

**Two-layer architecture works well.** Git watching provides baseline reactivity without daemon. lfd enhances with session tracking and live output when available. Clean separation means Maestro works even if lfd isn't running.

**Staleness threshold:** Hardcoded to 14 days. Could be configurable later.

**sync_main_repo refactored:** Simplified to always fetch first, then conditionally reset if branch is checked out. Cleaner than the previous branch-detection logic.
