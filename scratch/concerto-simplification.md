# Concerto Simplification

Reduce Concerto to its essential foundation before building embedded interactive sessions.

## Status: Complete

Both changes have been implemented:
1. ✓ Merged Wave and Worktree into one model (`Wave` in Swift, with worktree state enriched from daemon)
2. ✓ Split AppState into `RepoState`, `SessionState`, and `LauncherState`

## Context

The embedded terminal work (jack-heart.ghost.20260125_1034) revealed structural complexity that would compound as we build. Pre-PMF, concepts change daily. Better to simplify now than carry cruft forward.

## Change 1: Wave absorbs Worktree

### Problem (was)

Users see "waves" but the code maintained two parallel models:

```swift
// AppState had both
var waves: [Wave]
var worktrees: [Worktree]

// Every view did this dance
private var worktree: Worktree? {
    guard let path = wave.worktreePath else { return nil }
    return appState.worktrees.first { $0.path == path }
}
```

### Solution (implemented)

One model. `Wave` has everything:

```swift
public struct Wave: Sendable, Identifiable, Hashable {
    // Identity
    public let id: String
    public var name: String
    public let repo: String

    // Configuration
    public var area: [String]?
    public var direction: [String]?
    public var flow: String
    public var stimulus: Stimulus
    public var paused: Bool
    public var prLimit: Int
    public var mergeMode: MergeMode

    // Status
    public var status: WaveStatus
    public var iteration: Int
    public var pid: Int?

    // Worktree (was separate model)
    public var worktreePath: String?
    public var branch: String?
    public var isDirty: Bool
    public var isRebasing: Bool
    public var isMerging: Bool
    public var hasDiff: Bool
    public var aheadMain: Int
    public var behindMain: Int
    public var aheadRemote: Int
    public var behindRemote: Int

    // PR
    public var prURL: URL?
    public var prNumber: Int?
    public var prState: PRState?

    // Runtime
    public var staleness: Staleness
    public var recentSteps: [StepRun]
    public let createdAt: Date
}
```

### Deletions

| File | Status |
|------|--------|
| `WorktreeSidebar.swift` | ✓ Deleted |
| `WorktreeDetailPanel.swift` | ✓ Deleted |
| `AppState.swift` | ✓ Deleted |
| `AgentSidebar.swift` | ✓ Renamed to `WaveSidebar.swift` |
| `AgentDetailPanel.swift` | ✓ Renamed to `WaveDetailPanel.swift` |
| `AgentRow.swift` | ✓ Renamed to `WaveRow.swift` |
| `OutputPanel.swift` | ✓ Deleted |
| `ResultsPanel.swift` | ✓ Deleted |

## Change 2: Split AppState

### Problem (was)

AppState was 1,100 lines with 30+ properties spanning unrelated concerns.

### Solution (implemented)

Split into focused observable objects:

```swift
// Primary data - waves, flows, directions, config
@Observable final class RepoState {
    var currentRepo: URL?
    var config: LoopflowConfig?
    var waves: [Wave]
    var flows: [Flow]
    var directions: [Direction]
    var prompts: [PromptCard]
    var selectedWave: Wave?
    // ...
}

// Session tracking - running steps and their output
@Observable final class SessionState {
    var activeSessions: [String: ActiveSession] = [:]
    var interactiveSession: InteractiveSession?
    // ...
}

// Context assembly for prompt launching
@Observable final class LauncherState {
    var selectedPrompt: PromptCard?
    var selectedDirections: [Direction] = []
    var selectedModel: AgentModel?
    var promptArgs: String = ""
    // Context toggles...
}
```

### Injection

```swift
@main
struct ConcertoApp: App {
    @State private var repoState = RepoState()
    @State private var sessionState = SessionState()
    @State private var launcherState = LauncherState()

    var body: some Scene {
        WindowGroup {
            ContentView()
                .environment(repoState)
                .environment(sessionState)
                .environment(launcherState)
        }
    }
}
```

## Terminology changes

During implementation, naming evolved:

| Design doc term | Implemented as |
|-----------------|----------------|
| Agent | Wave |
| Goal | Direction |
| AgentService | WaveService |
| AgentSidebar | WaveSidebar |
| AgentDetailPanel | WaveDetailPanel |

## What remains

WorktreeService still handles git operations:
- `createWorktree()`, `removeWorktree()`
- `createPR()`, `landPR()`, `markReady()`
- `getDiff()`, `openInIDE()`, `openInTerminal()`

It's a git operations helper, not a data service.

## Next: Embedded terminal work

With simplified foundation, continue jack-heart.ghost.20260125_1034.
