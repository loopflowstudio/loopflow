# Worktree Dashboard

Replaces the worktree detail panel with a workflow-focused dashboard: quick actions bar, collapsible history/commits/diff sections, launcher collapsed by default.

## Review

**Verdict:** Ready to ship

The implementation matches the design intent. The dashboard surfaces what CLI-focused users need (history, commits, diff preview) while de-emphasizing the launcher. Quick actions for PR/Cursor/Warp/Diff are prominent.

Minor observations (non-blocking):

1. **Removed `ResultsSummaryView`** — The 184-line view was deleted entirely. This appears intentional since the new dashboard replaces that functionality, but worth confirming the session result display isn't needed elsewhere.

2. **Commit parsing robustness** — `getCommits` splits on `|` with `maxSplits: 4` but then checks `parts.count >= 5`. If a commit message contains `|`, this still works since the message is in `parts[2]` (before the author/date). Good.

3. **DiffSheet reuse** — The existing `DiffSheet` from `WorktreeSidebar.swift` is reused correctly. The sheet receives `diffContent` and `isLoading` state, allowing async loading.

## Design notes

**User intent (preserved):**
> "MVP for Maestro, focusing on workflow dashboard for someone using loopflow CLI to launch and loop agents, but not prioritizing the launcher path."

**Key decisions:**
- History expanded by default, commits/diff/launcher collapsed
- Section states persist via `@AppStorage`
- Diff preview truncated to 2000 chars with "Open Full" button
- Status badge inline with branch name (Running/Modified/Clean)
- Cached data (commits, diff) reset on worktree switch via `onChange(of: worktree.id)`
