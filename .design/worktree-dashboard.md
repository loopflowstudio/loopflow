# Worktree Dashboard

Workflow-focused dashboard replacing the worktree detail panel: quick actions bar, commits, GitHub-style file diff summary with per-file quick actions, launcher collapsed by default.

## Design

**Layout (top to bottom):**
1. Header — branch name, status badge, PR badge
2. Quick actions bar — Cursor, Warp, PR buttons (prominent)
3. Commits section — auto-expanded, shows branch commits
4. Changed Files section — GitHub-style diff summary sorted by changes
5. Launcher section — collapsed by default

**Changed Files display:**
- Summary bar showing total +additions/-deletions
- File list sorted by total changes (most changed first)
- Each file shows: icon, filename, directory, change bar (green/red blocks), stats
- Per-file quick actions: Cursor, Warp buttons
- Click file to view diff in sheet

**Key decisions:**
- History section removed (user feedback: not needed for workflow)
- Commits expanded by default, Files expanded by default
- Launcher collapsed by default (CLI users don't need it)
- Section states persist via `@AppStorage`
- PR button runs `lfops pr` which opens PR in browser

## Files

- `Maestro/Maestro/Models/FileDiffStat.swift` — file-level diff statistics
- `Maestro/Maestro/Services/WorktreeService.swift` — added `getDiffStats` method
- `Maestro/Maestro/Views/WorktreeDetailPanel.swift` — redesigned dashboard
