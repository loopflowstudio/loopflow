# Concerto Simplification

Reduce Concerto to its essential foundation before building embedded interactive sessions.

## Context

The embedded terminal work (jack-heart.ghost.20260125_1034) revealed structural complexity that will compound as we build. Pre-PMF, concepts change daily. Better to simplify now than carry cruft forward.

Two changes:
1. Merge Agent and Worktree into one model
2. Split AppState into focused state objects

## Change 1: Agent absorbs Worktree

### Problem

Users see "agents" but the code maintains two parallel models:

```swift
// AppState has both
var agents: [Agent]
var worktrees: [Worktree]

// Every view does this dance
private var worktree: Worktree? {
    guard let path = agent.worktreePath else { return nil }
    return appState.worktrees.first { $0.path == path }
}
```

Agent has config (flow, goal, area, stimulus). Worktree has state (commits, PR, CI, dirty). But they're the same thing—an agent IS a worktree with config.

### Solution

One model. Agent has everything:

```swift
struct Agent: Codable, Identifiable, Hashable {
    // Identity
    let id: String
    var name: String
    let repo: String

    // Configuration
    var area: [String]?
    var goal: [String]?
    var flow: String
    var stimulus: Stimulus
    var paused: Bool
    var prLimit: Int
    var mergeMode: MergeMode

    // Status
    var status: AgentStatus
    var iteration: Int
    var pid: Int?
    var consecutiveFailures: Int
    var pendingActivations: Int

    // Worktree (was separate model)
    let path: String                    // non-optional, agents always have worktrees
    var branch: String
    var baseBranch: String?
    var isDirty: Bool
    var isRebasing: Bool
    var isMerging: Bool
    var hasDiff: Bool
    var aheadMain: Int
    var behindMain: Int
    var aheadRemote: Int
    var behindRemote: Int

    // PR
    var prURL: URL?
    var prNumber: Int?
    var prState: PRState?

    // Runtime
    var ciStatus: CIStatus?
    var staleness: Staleness
    var recentSteps: [StepRun]

    // Timestamps
    let createdAt: Date
}
```

### Daemon API change

The daemon enriches `/agents` response with worktree state:

```python
# http_server.py: handle_agents_list
def handle_agents_list(repo: Path) -> dict:
    agents = list_agents(repo)
    enriched = []
    for agent in agents:
        data = agent.model_dump()
        if agent.worktree:
            wt_state = get_worktree_state(agent.worktree)  # git status, PR info
            data.update(wt_state)
        enriched.append(data)
    return {"agents": enriched}
```

Remove `/worktrees` endpoint. WorktreeService becomes internal implementation detail for git operations, not a data source.

### Swift deletions

| File | Action |
|------|--------|
| `Worktree.swift` | Delete |
| `WorktreeSidebar.swift` | Delete (agents section moves to AgentSidebar) |
| `WorktreeDetailPanel.swift` | Delete (merge into AgentDetailPanel) |
| `AppState.worktrees` | Delete |
| `AppState.selectedWorktree` | Delete (use selectedAgent) |
| `WorktreeService` worktree-fetching methods | Delete |

### What WorktreeService keeps

WorktreeService still handles git operations:
- `createWorktree()`, `removeWorktree()`
- `createPR()`, `landPR()`, `markReady()`
- `getDiff()`, `openInIDE()`, `openInTerminal()`

It becomes a git operations helper, not a data service.

## Change 2: Split AppState

### Problem

AppState is 1,100 lines with 30+ properties spanning unrelated concerns:

```swift
// Data
var agents: [Agent]
var flows: [Flow]
var goals: [Goal]
var prompts: [PromptCard]

// 8 session tracking dictionaries
var liveOutputBySession: [String: [OutputLine]]
var activeSessionIds: Set<String>
var stepRunBaselines: [String: StepRunBaseline]
var stepRunResults: [String: StepRunResult]
private var stepRunWorktreeMap: [String: String]
private var stepRunStepMap: [String: String]
private var stepRunStartMap: [String: Date]
var activeWorktreePaths: Set<String>

// 15+ prompt launcher properties
var selectedPrompt: PromptCard?
var selectedGoals: [Goal]
var promptArgs: String
var includeDocs: Bool
// ... etc

// UI state
var selectedAgent: Agent?
var activeSession: InteractiveSession?
var isLoading: Bool
var errorMessage: String?
```

Session cleanup touches 7 maps. A change to any property triggers observation updates across the app.

### Solution

Split into focused observable objects:

```swift
// Primary data - agents, flows, goals, config
@Observable final class RepoState {
    var currentRepo: URL?
    var config: LoopflowConfig?
    var agents: [Agent]
    var flows: [Flow]
    var goals: [Goal]
    var prompts: [PromptCard]

    // Loading
    var isLoading: Bool = false
    var errorMessage: String?

    // Selection
    var selectedAgent: Agent?
}

// Session tracking - running steps and their output
@Observable final class SessionState {
    var activeSessions: [String: ActiveSession] = [:]

    struct ActiveSession {
        let id: String
        let agentId: String
        let step: String
        var baseline: StepRunBaseline?
        var output: [OutputLine]
        var result: StepRunResult?
        let startedAt: Date
    }

    // Interactive session (one at a time for MVP)
    var interactiveSession: InteractiveSession?
}

// Context assembly for prompt launching
@Observable final class LauncherState {
    var selectedPrompt: PromptCard?
    var selectedGoals: [Goal] = []
    var selectedModel: AgentModel?
    var promptArgs: String = ""

    // Context toggles
    var includeDocs: Bool = true
    var includeDiff: Bool = true
    var includeDiffFiles: Bool = false
    var includePaste: Bool = false
    var includeSummaries: Bool = false
    var includeChrome: Bool = false
    var selectedContextFolders: Set<URL> = []
    var attachedFiles: [URL] = []
    var excludedFiles: Set<String> = []

    var runMode: RunMode = .auto
    var estimatedTokens: Int = 0
    var contextPreview: ContextPreview = ContextPreview()
}
```

### Injection approach

Use `@Environment` for app-wide state:

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

// Views take only what they need
struct AgentSidebar: View {
    @Environment(RepoState.self) private var repoState

    var body: some View {
        List(repoState.agents) { agent in ... }
    }
}

struct ResultsPanel: View {
    @Environment(SessionState.self) private var sessionState

    var body: some View {
        ForEach(sessionState.activeSessions.values) { ... }
    }
}
```

### Session lifecycle simplification

Before (7 operations):
```swift
activeSessionIds.remove(id)
stepRunWorktreeMap.removeValue(forKey: id)
stepRunStepMap.removeValue(forKey: id)
stepRunStartMap.removeValue(forKey: id)
liveOutputBySession.removeValue(forKey: id)
// result computed separately
stepRunResults[id] = computeResult(...)
```

After (1 operation):
```swift
sessionState.activeSessions[id]?.result = computeResult(...)
// or to end and archive:
sessionState.activeSessions.removeValue(forKey: id)
```

### Services

Services stay as they are—they're not state, they're operations. They can take state objects as parameters:

```swift
class AgentService {
    func refresh(into state: RepoState) async throws { ... }
    func run(agent: Agent, from state: RepoState) async throws { ... }
}
```

Or access them via environment in views that coordinate.

## Migration path

### Phase 1: Merge Agent/Worktree (Python + Swift)

1. Add worktree state fields to daemon Agent response
2. Update Swift Agent model to include all fields
3. Update AgentService to parse enriched response
4. Remove worktrees array from AppState
5. Delete Worktree.swift, WorktreeSidebar.swift, WorktreeDetailPanel.swift
6. Update all views to use Agent directly
7. Remove /worktrees endpoint from daemon

### Phase 2: Split AppState (Swift only)

1. Create RepoState, SessionState, LauncherState
2. Move properties from AppState to appropriate new objects
3. Update environment injection in app entry point
4. Update views to take specific state objects
5. Delete AppState.swift (or rename to legacy if gradual migration)

### Phase 3: Resume embedded terminal work

With simplified foundation, continue jack-heart.ghost.20260125_1034.

## Verification

After each phase, run existing tests. Some may break if they relied on the old structure—update or delete as appropriate.

Manual test:
- [ ] Agent list shows with correct status, commits, PR info
- [ ] Agent detail shows worktree state (dirty, ahead/behind, CI)
- [ ] Run agent → session appears in results
- [ ] Session completes → result computed correctly
- [ ] Prompt launcher still works

## Deletions summary

| Category | Files/Code |
|----------|------------|
| Models | Worktree.swift |
| Views | WorktreeSidebar.swift, WorktreeDetailPanel.swift |
| State | AppState.worktrees, 6 session dictionaries |
| API | /worktrees endpoint |
| Service methods | WorktreeService list/fetch methods |

## Not in scope

- Renaming Agent to something else
- Changing the daemon's internal Agent model
- Multi-session support (stays one-at-a-time)
- Persisting session state across app restart
