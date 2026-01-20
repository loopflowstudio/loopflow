# Maestroux

Live git watching and stale worktree detection for Maestro, without requiring lfd.

## Review

**Verdict:** Ready to ship

No blocking issues. Implementation matches design for Phases 1, 2, and 4.

## Implementation Summary

**GitWatcherService** - FSEvents-based file system watcher that monitors `.git/worktrees/`, `.git/refs/`, and `.git/index`. Debounces at 500ms. Handles both regular repos and worktrees (resolves `.git` files pointing to real git directories).

**LFDEventService** - Updated with graceful connection handling. Checks socket existence before connecting, adds 5-second reconnect loop, exposes `isConnected` state via callback.

**Staleness detection** - `Staleness` enum with four states: `.active`, `.merged`, `.remoteDeleted`, `.inactive(days:)`. Detection runs async after worktree refresh. Checks merged branches, remote refs, and commit age (14 day threshold).

**UI** - Connection indicator in sidebar header (green dot = lfd connected, gray = file watcher only). Stale worktrees show badges with appropriate icons and dimmed text.

## Design Notes

**Two-layer architecture works well.** Git watching provides baseline reactivity without daemon. lfd enhances with session tracking and live output when available. Clear separation.

**Phases not implemented:**
- Phase 3 (working directory watching) - not needed for initial release
- Phase 5 (auto-pruning) - stale detection is sufficient for now, pruning can be manual

**Staleness threshold hardcoded to 14 days.** Could be configurable later if needed.
