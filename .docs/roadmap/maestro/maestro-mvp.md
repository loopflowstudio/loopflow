# Maestro MVP

**Status:** proposed
**Area:** maestro
**Priority:** high

## Problem

Users have no visibility into what loopflow is doing. When running `lfd` loops or `lf` flows, output goes to terminal logs. There's no single place to see:

- What agents are running
- What worktrees exist
- What tasks are queued or completed
- Live output from auto sessions

The vision documents describe Maestro as "the podium"—where you conduct from. But today, there's no podium.

## Proposal

Build a minimal macOS native app that shows:

1. **Worktree list** — all worktrees in the current project, their branches, and status
2. **Agent status** — running loops (`lfd`), their current state, last output
3. **Ship button** — launch `lfops pr` or `lfops land` with one click

### What this is NOT

- Not an embedded terminal (use Warp/Ghostty)
- Not an IDE (use Cursor/VS Code)
- Not a code viewer (use your editor)

### Design principles

- Read-only view of state (except ship actions)
- Polls filesystem/git for state—no daemon required
- Native macOS (Swift/SwiftUI)
- Single window, minimal chrome

## Success criteria

- Launch Maestro, see all worktrees in current project
- See which loops are running and their last output timestamp
- Click "Ship" → PR opens in browser

## Open questions

1. How to detect running loops? Poll `lfd status` or read state files?
2. Where does Maestro live—menu bar app or dock app?
3. Should it support multiple projects simultaneously?

## Dependencies

- `lfd status` command (exists)
- Worktree detection (exists via git)
- `lfops pr` integration (exists)

## Effort

Small: 2-3 focused sessions to get a working prototype.
