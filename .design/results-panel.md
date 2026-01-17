# Results Panel

Transform OutputPanel from streaming log viewer into outcome-focused results panel.

**Status**: Implemented. Build passes.

## Problem

The OutputPanel duplicates what users see in their terminal. When a task runs, output streams to both Maestro and the terminal window—two views of the same data. The terminal is better for streaming logs; Maestro should show what matters: what changed.

From ux-gaps.md:
> "The app that received input is not the app that shows output. This is the fundamental architectural tension."

The recommendation: "Don't try to be a terminal. Show what matters: what changed, whether it worked, what to do next."

## What to build

Replace the streaming OutputPanel with a results summary that appears when a session completes. Show:

1. **Files changed** — list of modified/added/deleted files with line counts
2. **Diff preview** — inline diff view (reuse existing DiffContentView)
3. **Test results** — pass/fail count if detected from output
4. **Duration** — how long the task took
5. **Actions** — "Open in Terminal" for full logs, "View Diff" for full diff sheet

While a task is running, show a minimal progress indicator ("Running design...") instead of streaming the full log.

## Data flow

### On session start
1. Session starts → `session.started` event includes worktree path
2. Capture the git state before: `git rev-parse HEAD` and `git diff --stat`
3. Show minimal running indicator with task name

### On session complete
1. Session ends → `session.ended` event with status
2. Capture the git state after: new commits, changed files
3. Compute diff between before/after states
4. Display results summary

### Data structure

```swift
struct SessionResult: Identifiable {
    let id: String  // session ID
    let task: String
    let worktree: String
    let status: SessionStatus  // completed, error
    let duration: TimeInterval
    let filesChanged: [FileChange]
    let newCommits: [String]  // commit messages
    let testsPassed: Int?
    let testsFailed: Int?
}

struct FileChange: Identifiable {
    let id = UUID()
    let path: String
    let kind: FileChangeKind  // added, modified, deleted
    let linesAdded: Int
    let linesRemoved: Int
}

enum FileChangeKind {
    case added, modified, deleted
}
```

## UI

### While running

```
┌─────────────────────────────────────────────────┐
│ ● Running design...                     1m 23s  │
└─────────────────────────────────────────────────┘
```

Minimal bar at bottom with pulsing dot, task name, and elapsed time.

### After completion

```
┌─────────────────────────────────────────────────┐
│ ✓ design completed                       2m 14s │
├─────────────────────────────────────────────────┤
│ 3 files changed                                 │
│                                                 │
│ ▼ src/auth.py                        +47 -12   │
│   def login(email, password):                   │
│   +    """Authenticate user with email..."""    │
│   +    user = db.find_user(email)               │
│   ...                                           │
│                                                 │
│ ▸ src/models.py                      +23 -0    │
│ ▸ tests/test_auth.py                 +31 -0    │
│                                                 │
│ 1 new commit: "Add email authentication"        │
│                                                 │
│ [Open Terminal]  [View Full Diff]               │
└─────────────────────────────────────────────────┘
```

- Collapsible file entries (▼/▸)
- Inline diff preview for expanded files (first ~10 lines)
- Commit messages shown
- Action buttons at bottom

### On error

```
┌─────────────────────────────────────────────────┐
│ ✗ implement failed                       4m 02s │
├─────────────────────────────────────────────────┤
│ Task ended with error                           │
│                                                 │
│ 2 files changed before failure                  │
│                                                 │
│ ▸ src/auth.py                        +12 -3    │
│ ▸ src/config.py                      +4 -0     │
│                                                 │
│ [Open Terminal]  [View Full Diff]               │
└─────────────────────────────────────────────────┘
```

Show partial results if there were changes before the error.

## Implementation

### 1. Add ResultsService

New service that:
- Captures git state when session starts (worktree HEAD, dirty files)
- Computes diff when session ends (new commits, file changes)
- Parses output for test results (optional pattern matching)

```swift
class ResultsService {
    func captureBaseline(worktree: URL) async -> SessionBaseline
    func computeResults(baseline: SessionBaseline, worktree: URL) async -> SessionResult
}

struct SessionBaseline {
    let worktree: String
    let headSHA: String
    let dirtyFiles: [String]
    let timestamp: Date
}
```

### 2. Update AppState

Track session baselines and results:

```swift
// In AppState
var sessionBaselines: [String: SessionBaseline] = [:]  // session ID → baseline
var sessionResults: [String: SessionResult] = [:]  // session ID → result
var selectedResultSessionId: String?  // for multi-session UI
```

### 3. Replace OutputPanel

New `ResultsPanel` view that shows:
- Running indicator while active
- Results summary when complete
- Reuses `DiffContentView` for inline previews

### 4. Update event handling

In `handleSessionEvent`:
- On `session.started`: capture baseline via ResultsService
- On `session.ended`: compute results, store in AppState

## Constraints

- **Git operations only** — Don't parse terminal output for file changes; use git commands directly. More reliable.
- **Lazy diff loading** — Don't load full diffs upfront. Load per-file when user expands.
- **Keep streaming option** — Add a "Show Log" toggle for users who want the old behavior. Default to results view.
- **Handle no changes** — If task completed but nothing changed, show "No changes" with explanation.

## Done when

1. OutputPanel replaced with ResultsPanel
2. Running state shows minimal indicator with task name and timer
3. Completed state shows files changed, diff previews, commit messages
4. Error state shows partial results
5. "Open Terminal" and "View Full Diff" buttons work
6. Old streaming output available via toggle (non-default)

## Out of scope

- Test result parsing (complex, agent-output-dependent)
- PR creation from results panel (separate feature)
- Result history beyond current session (use worktree context menu)
- Notifications on completion (separate feature)
