# Worktree Dashboard MVP

## What to build

Replace the worktree detail panel with a workflow-focused dashboard showing quick actions, branch state, and task history—with the launcher collapsed by default.

## User intent

> "MVP for Maestro, focusing on workflow dashboard for someone using loopflow CLI to launch and loop agents, but not prioritizing the launcher path. Still, there should be a way to launch, but it can be hidden behind a click on the main dashboard."

> "Will include jump to PR, open in Cursor, open in Warp, see current diff, see commit messages for worktree (default off), see lf history for worktree (default on)"

## Data structures

```swift
// Extend Worktree to include commits (already has recentTasks for history)
struct CommitInfo: Identifiable, Equatable {
    let id: String  // SHA
    let shortSHA: String
    let message: String
    let author: String
    let date: Date
}

// Add to WorktreeService
func getCommits(for worktree: Worktree, since: String = "main") async throws -> [CommitInfo]
func getDiff(_ spec: String, in repo: URL) async throws -> String  // already exists
```

## Key functions

```swift
// WorktreeService additions
func getCommits(for worktree: Worktree, since: String = "main") async throws -> [CommitInfo]
    /// git log main..HEAD --oneline --format="%H|%h|%s|%an|%aI"

// AppState additions
func loadWorktreeHistory(_ worktree: Worktree) async -> [TaskSession]
    /// Calls SessionService.history(for: worktree.path)
```

## UI changes

### WorktreeDetailPanel (redesigned)

```
┌─────────────────────────────────────────────────────────┐
│  feature-auth                    ○ Clean               │
│  PR #42 (Open) · 3 ahead                               │
├─────────────────────────────────────────────────────────┤
│  [🔗 PR]  [📁 Cursor]  [⌨ Warp]  [📋 Diff]             │
├─────────────────────────────────────────────────────────┤
│                                                         │
│  ▼ History (default on)                                │
│    ● implement  2m ago  completed                      │
│    ● review     15m ago completed                      │
│    ● design     1h ago  completed                      │
│                                                         │
│  ▶ Commits (default off)                               │
│                                                         │
│  ▶ Diff Preview (default off)                          │
│                                                         │
├─────────────────────────────────────────────────────────┤
│  ▶ Launcher                                            │
└─────────────────────────────────────────────────────────┘
```

### Components

**Header** (same as current)
- Branch name, status indicator (clean/dirty/running)
- PR badge if exists, commits ahead/behind

**Quick Actions Bar** (new)
- Jump to PR (opens GitHub) — only if PR exists
- Open in Cursor
- Open in Warp
- View Diff (opens diff sheet — reuse existing DiffSheet)

**Collapsible Sections** (new)

| Section | Default | Content |
|---------|---------|---------|
| History | expanded | Task sessions from `SessionService`, shows task name, relative time, status badge |
| Commits | collapsed | Commits on branch since main, shows short SHA, message, time |
| Diff Preview | collapsed | Inline diff view (truncated), "Open Full" button triggers DiffSheet |

**Launcher** (existing, moved to bottom)
- Collapsed by default with chevron toggle
- Same functionality as current `CollapsedLauncher`

### View hierarchy

```swift
struct WorktreeDetailPanel: View {
    // Header
    header                    // branch name, PR badge, status

    // Quick actions
    quickActionsBar           // PR, Cursor, Warp, Diff buttons

    Divider()

    // Scrollable content
    ScrollView {
        HistorySection(...)   // expanded by default
        CommitsSection(...)   // collapsed by default
        DiffPreviewSection(...) // collapsed by default
    }

    Divider()

    // Launcher at bottom
    launcherSection           // collapsed by default
}
```

## Constraints

- **SessionService dependency**: History requires `~/.lf/lfd.db` to exist. If not available, show "No history yet—run a task to see it here."
- **Commits require git**: Use existing `WorktreeService` pattern with `Process` calls.
- **PR button visibility**: Only show "Jump to PR" when `worktree.prURL != nil`.
- **Section state persistence**: Use `@AppStorage` for collapsed state so it persists across sessions.

## Done when

1. Select a worktree in sidebar → detail panel shows dashboard (not launcher)
2. Quick action buttons work:
   - PR button opens `worktree.prURL` in browser
   - Cursor button opens worktree in Cursor
   - Warp button opens worktree in Warp
   - Diff button opens `DiffSheet`
3. History section shows task sessions from lfd.db (or empty state if no db)
4. Commits section shows commits on branch (collapsed by default)
5. Diff preview section shows truncated diff (collapsed by default)
6. Launcher section at bottom, collapsed by default, fully functional when expanded

**Verification:**
```bash
# Build and run
cd Maestro && xcodebuild -scheme Maestro -configuration Debug build

# Manual test:
# 1. Open Maestro, select a worktree
# 2. Verify quick actions bar is visible
# 3. Click Warp → terminal opens at worktree path
# 4. Click Cursor → Cursor opens at worktree path
# 5. History section shows task sessions (or empty state)
# 6. Expand Commits → shows git log
# 7. Expand Launcher → can run tasks
```
