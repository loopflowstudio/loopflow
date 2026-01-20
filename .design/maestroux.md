# Live Reactivity for Maestro

Make worktrees, diffs, and commit history update in real-time. Auto-prune stale worktrees. Work without lfd.

## Goals

1. **Live worktree list** - new/deleted worktrees appear immediately
2. **Live diff** - file changes reflected without manual refresh
3. **Live commit history** - new commits appear as they land
4. **Stale worktree detection** - highlight merged/abandoned branches
5. **Auto-pruning** - clean up worktrees after merge queue completes
6. **No lfd required** - core features work without the daemon

## Non-Goals

- Real-time collaboration (multi-user)
- Cross-repo synchronization
- Persistent state between app launches

## Current State

| Feature | Source | Reactivity |
|---------|--------|------------|
| Worktree list | `wt list` CLI | lfd `worktree.*` events only |
| Diff | `git diff` on-demand | None |
| Commit history | lfd sessions DB | lfd `session.*` events only |
| Live output | lfd socket stream | Real-time |

**Problem:** Without lfd running, nothing updates automatically.

## Design

### Two-Layer Architecture

```
┌─────────────────────────────────────────────────────────┐
│                      Maestro                            │
├─────────────────────────────────────────────────────────┤
│  GitWatcherService (always on)                          │
│    - FSEvents on .git/worktrees/, refs/, index          │
│    - Debounce → refresh worktrees, diff, commits        │
├─────────────────────────────────────────────────────────┤
│  LFDEventService (optional, enhances)                   │
│    - Session lifecycle, live output                     │
│    - Git hook events (immediate, no polling)            │
│    - Reconnects on daemon restart                       │
├─────────────────────────────────────────────────────────┤
│  AppState                                               │
│    - Merges events from both sources                    │
│    - Deduplicates rapid-fire updates                    │
└─────────────────────────────────────────────────────────┘
```

### Layer 1: GitWatcherService (Maestro-native)

FSEvents-based file system watcher. No dependencies.

**Watch targets:**

| Path | Triggers | Debounce |
|------|----------|----------|
| `.git/worktrees/` | Worktree list refresh | 500ms |
| `.git/refs/heads/` | Commit history refresh | 500ms |
| `.git/index` | Diff refresh (staging) | 200ms |
| `<worktree>/**` | Diff refresh (working dir) | 500ms |

**Implementation notes:**

- Use `DispatchSource.makeFileSystemObjectSource` or FSEventStream API
- Watch the main repo's `.git/` directory
- For each worktree, also watch its working directory for file changes
- Coalesce rapid events (git operations touch many files)
- Ignore `.git/objects/` (too noisy, not useful for UI)

**Swift sketch:**

```swift
actor GitWatcherService {
    private var sources: [DispatchSourceFileSystemObject] = []
    private var debounceTask: Task<Void, Never>?

    enum ChangeKind {
        case worktrees
        case refs
        case index
        case workingDir(path: String)
    }

    func watch(repo: URL, onChange: @escaping (ChangeKind) -> Void) {
        let gitDir = repo.appendingPathComponent(".git")

        // Watch .git/worktrees/ for worktree changes
        watchDirectory(gitDir.appendingPathComponent("worktrees")) {
            onChange(.worktrees)
        }

        // Watch .git/refs/heads/ for branch/commit changes
        watchDirectory(gitDir.appendingPathComponent("refs/heads")) {
            onChange(.refs)
        }

        // Watch .git/index for staging changes
        watchFile(gitDir.appendingPathComponent("index")) {
            onChange(.index)
        }
    }

    private func watchDirectory(_ url: URL, onChange: @escaping () -> Void) {
        // FSEvents or DispatchSource implementation
    }
}
```

### Layer 2: LFDEventService (Enhanced, Optional)

Current implementation already handles:
- `session.started` / `session.ended`
- `output.line` streaming
- `worktree.*` from git hooks

**Changes needed:**

1. **Graceful connection handling** - don't fail on startup if lfd isn't running
2. **Reconnection** - periodically try to connect if disconnected
3. **Connection state** - expose `isConnected` for UI indicator

```swift
actor LFDEventService {
    var isConnected: Bool { connection?.state == .ready }

    func connect() async {
        // Try to connect, return silently if socket doesn't exist
        guard FileManager.default.fileExists(atPath: socketPath.path) else {
            return
        }
        // ... existing connection logic
    }

    func startReconnectLoop() {
        Task {
            while true {
                try? await Task.sleep(for: .seconds(5))
                if !isConnected {
                    await connect()
                }
            }
        }
    }
}
```

### AppState Integration

Merge events from both sources:

```swift
@Observable
final class AppState {
    private let gitWatcher = GitWatcherService()
    private let lfdService = LFDEventService()

    var lfdConnected: Bool = false  // For UI indicator

    func openRepo(_ url: URL) async {
        // Start native git watching (always)
        await gitWatcher.watch(repo: url) { [weak self] change in
            Task { @MainActor in
                self?.handleGitChange(change)
            }
        }

        // Try to connect to lfd (optional enhancement)
        await lfdService.connect()
        lfdService.startReconnectLoop()

        // ... rest of openRepo
    }

    private func handleGitChange(_ change: GitWatcherService.ChangeKind) {
        switch change {
        case .worktrees:
            Task { await refreshWorktrees() }
        case .refs:
            Task { await refreshCommitHistory() }
        case .index, .workingDir:
            Task { await refreshDiff() }
        }
    }
}
```

### Stale Worktree Detection

Add to `Worktree` model:

```swift
struct Worktree {
    // ... existing fields

    var staleness: Staleness = .active

    enum Staleness {
        case active              // Has recent commits or activity
        case merged              // Branch merged to main
        case remoteDdeleted      // Remote tracking branch gone
        case inactive(days: Int) // No commits in N days
    }
}
```

**Detection logic in WorktreeService:**

```swift
func detectStaleness(for worktree: Worktree, in repo: URL) async -> Worktree.Staleness {
    // Check if branch is merged
    let mergedOutput = try? await run("git", "branch", "--merged", "main", in: repo)
    if mergedOutput?.contains(worktree.branch) == true {
        return .merged
    }

    // Check if remote tracking branch exists
    let remoteRef = "refs/remotes/origin/\(worktree.branch)"
    let refExists = try? await run("git", "show-ref", "--verify", remoteRef, in: repo)
    if refExists == nil && worktree.pr != nil {
        // Had a PR but remote branch is gone → merged and cleaned up
        return .remoteDeleted
    }

    // Check last commit date
    let lastCommit = try? await run(
        "git", "log", "-1", "--format=%ct", worktree.branch, in: repo
    )
    if let timestamp = lastCommit.flatMap(Int.init) {
        let age = Date().timeIntervalSince1970 - Double(timestamp)
        let days = Int(age / 86400)
        if days > 14 {
            return .inactive(days: days)
        }
    }

    return .active
}
```

### Auto-Pruning

Two triggers for auto-prune:

1. **On refresh** - check staleness, prompt user for stale worktrees
2. **Merge queue completion** - watch for branch deletion on origin

**Merge queue detection:**

When a worktree's PR is in merge queue:
1. Watch `refs/remotes/origin/<branch>`
2. When it disappears → PR merged, branch deleted
3. Prompt: "Branch X was merged. Remove worktree?"

Or automatic mode (user preference):

```swift
@AppStorage("autoPruneAfterMerge") var autoPruneAfterMerge = false

func handleRefChange() async {
    for worktree in worktrees where worktree.pr?.inMergeQueue == true {
        let remoteBranchExists = await checkRemoteBranch(worktree.branch)
        if !remoteBranchExists {
            if autoPruneAfterMerge {
                try? await deleteWorktree(worktree)
            } else {
                // Mark for UI highlight
                pendingPrunes.insert(worktree.id)
            }
        }
    }
}
```

## Implementation Plan

### Phase 1: GitWatcherService

1. Create `GitWatcherService.swift` with FSEvents-based watching
2. Watch `.git/worktrees/`, `.git/refs/heads/`, `.git/index`
3. Add debouncing (500ms default)
4. Integrate into AppState.openRepo()
5. Test: create worktree via CLI, verify UI updates

### Phase 2: Graceful lfd Handling

1. Update `LFDEventService` to handle missing socket gracefully
2. Add reconnection loop (5s interval)
3. Expose `isConnected` state
4. Add UI indicator showing lfd status
5. Test: start app without lfd, start lfd, verify connection

### Phase 3: Working Directory Watching

1. Extend GitWatcherService to watch worktree working directories
2. Ignore common noise (node_modules, .git/objects, build artifacts)
3. Trigger diff refresh on file changes
4. Add per-worktree watch lifecycle (add watch when worktree selected)
5. Test: edit file, verify diff updates

### Phase 4: Stale Detection

1. Add `Staleness` enum to Worktree model
2. Implement detection logic in WorktreeService
3. Run detection on refresh (async, don't block UI)
4. Add visual indicators in WorktreeSidebar
5. Test: merge a PR, verify worktree shows as stale

### Phase 5: Auto-Pruning

1. Add "Prune Stale" button in sidebar
2. Add user preference for auto-prune after merge
3. Implement merge queue completion detection
4. Add confirmation dialog (or auto-prune if preference set)
5. Test: merge via merge queue, verify worktree removed

## Open Questions

1. **Working dir watch scope** - Watch entire worktree or just top-level? Deep watching could be expensive for large repos.

2. **Staleness threshold** - How many days of inactivity = stale? Should this be configurable?

3. **Auto-prune safety** - Should we require the worktree to be clean (no uncommitted changes) before auto-pruning?

4. **Multiple repos** - If user has multiple repos open, do we watch all of them? Memory/CPU implications?

5. **lfd indicator placement** - Where should the "lfd connected" indicator live in the UI?

## Files to Create/Modify

**New:**
- `Maestro/Maestro/Services/GitWatcherService.swift`

**Modify:**
- `Maestro/Maestro/Services/LFDEventService.swift` - graceful connection, reconnect
- `Maestro/Maestro/Models/Worktree.swift` - staleness enum
- `Maestro/Maestro/Services/WorktreeService.swift` - staleness detection
- `Maestro/Maestro/AppState.swift` - integrate watchers, prune logic
- `Maestro/Maestro/Views/WorktreeSidebar.swift` - staleness indicators, prune UI
