# Simplification Opportunities

## Product intent

Concerto is a podium for conducting AI coding agents. Users create agents, watch them work, and review their output (PRs). The core loop: configure agent → run → review changes → land or iterate.

## Opportunity 1: Agent and Worktree are the same thing

**Misalignment**: The product presents "agents" but the implementation has two separate concepts—Agent (configuration + status) and Worktree (git state + PR info). Users see agents in the sidebar, but the detail panel shows worktree data (branch name, commits, changed files).

**Symptom**: Throughout the codebase, agent operations require worktree lookups:

```swift
// AgentSidebar.swift:53-60
private func pendingPR(for agent: Agent) -> (number: Int, url: URL?)? {
    guard let path = agent.worktreePath,
          let worktree = appState.worktrees.first(where: { $0.path == path }),
          // ...
}

// AgentDetailPanel.swift:25
private var worktree: Worktree? {
    guard let path = agent.worktreePath else { return nil }
    return appState.worktrees.first { $0.path == path }
}
```

Every view that shows agent details must do this lookup. The Agent model has worktree fields (branch, prLimit, mergeMode) while Worktree has overlapping fields (branch, prNumber, prState). AppState maintains both `agents: [Agent]` and `worktrees: [Worktree]` arrays that must stay synchronized.

**Realignment**: An Agent IS a Worktree with additional configuration. Merge them:

```swift
struct Agent {
    // Identity
    let id: String
    var name: String

    // Configuration (agent-specific)
    var area: [String]?
    var goal: [String]?
    var flow: String
    var stimulus: Stimulus

    // Runtime state (currently on Worktree)
    var branch: String
    var path: String
    var isDirty: Bool
    var commits: Int
    var prNumber: Int?
    var prState: PRState?
    var ciStatus: CIStatus?
    // ...
}
```

One array in AppState: `agents: [Agent]`. Remove `worktrees` entirely—it's an implementation detail that leaked into the UI layer.

**Cascade**:
- Delete WorktreeService, WorktreeSidebar, WorktreeDetailPanel (already marked as replaced but still exist)
- Remove all `worktrees.first(where:)` lookups
- Simplify AgentSidebar section logic (currently computes pendingPR separately)
- Single refresh path: `refreshAgents()` fetches everything
- Agent model becomes self-contained; views don't need AppState for lookups

## Opportunity 2: AppState is a god object

**Misalignment**: AppState manages everything—data, UI state, session tracking, results, context preview. It's 1,100+ lines with 30+ properties spanning unrelated concerns. The product has clear feature boundaries (agent management, prompt launching, session results) but the state doesn't reflect them.

**Symptom**: Seven interlocking dictionaries track session results:

```swift
var stepRunBaselines: [String: StepRunBaseline]
var stepRunResults: [String: StepRunResult]
private var stepRunWorktreeMap: [String: String]
private var stepRunStepMap: [String: String]
private var stepRunStartMap: [String: Date]
var liveOutputBySession: [String: [OutputLine]]
var activeSessionIds: Set<String>
```

When a session ends, cleanup must touch all seven:

```swift
// AppState.swift:719-745 - handleSessionEvent
activeSessionIds.remove(event.id)
if let worktree = stepRunWorktreeMap.removeValue(forKey: event.id) { ... }
stepRunStepMap.removeValue(forKey: event.id)
stepRunStartMap.removeValue(forKey: event.id)
```

**Realignment**: Split AppState into feature-scoped observable objects:

```swift
// Repo and primary data
@Observable class RepoState {
    var currentRepo: URL?
    var config: LoopflowConfig?
    var agents: [Agent]
    var flows: [Flow]
    var goals: [Goal]
}

// Session tracking (currently 7 dictionaries → 1)
@Observable class SessionState {
    var activeSessions: [String: ActiveSession]

    struct ActiveSession {
        let id: String
        let agentId: String
        let step: String
        var baseline: StepRunBaseline?
        var output: [OutputLine]
        var result: StepRunResult?
    }
}

// Prompt launcher (15+ properties → 1 object)
@Observable class PromptLauncherState {
    var selectedPrompt: PromptCard?
    var selectedGoals: [Goal]
    var args: String
    var contextOptions: ContextOptions
    var preview: ContextPreview
}
```

**Cascade**:
- Views take only the state they need (AgentSidebar takes RepoState, ResultsPanel takes SessionState)
- Session lifecycle becomes atomic—create/update/remove one ActiveSession instead of 7 map operations
- Prompt launcher becomes testable in isolation
- State changes trigger fewer re-renders (updates scoped to feature)

## Opportunity 3: Interactive vs auto mode exists in UI but agents only run auto

**Misalignment**: The product vision includes interactive sessions (design doc: "Run in embedded terminal with interactive input"). The UI has `InteractiveSession`, `InteractiveSessionView`, and an "Interactive" toggle in FlowPicker. But the daemon only runs flows in auto mode—agents never spawn interactive sessions.

**Symptom**: FlowPicker shows an interactive toggle that does nothing for agents:

```swift
// FlowPicker.swift
Toggle(isOn: $isInteractive) {
    Text("Interactive")
}
// ...
private func runExperiment() {
    try await appState.runAgent(
        agent: agent,
        flow: selectedFlowName,
        stimulus: Stimulus(kind: .once)  // Always auto
    )
}
```

Meanwhile, `launchInteractiveSession()` exists but only works for the embedded terminal (a separate code path). The two modes don't share infrastructure.

**Realignment**: Either commit to interactive agents or remove the toggle.

Option A (commit): Make `runAgent()` support interactive mode by launching the step in the embedded terminal instead of via subprocess. The daemon doesn't need to know—interactive runs are local.

Option B (remove): Delete the toggle and InteractiveSession machinery. Agents run auto; users who want interactive run `lf design -i` in their terminal.

The current halfway state—UI suggests interactive but implementation ignores it—confuses users.

**Cascade**:
- If Option A: InteractiveSessionView replaces auto output for running agents
- If Option B: Delete InteractiveSession, InteractiveSessionView, toggle from FlowPicker
- Either way: one execution path, not two

## Aligned areas

**Agent attention hierarchy**: The sidebar organizes agents by urgency (Needs Attention → Open PRs → Active → Idle). This matches how users actually work—check blocked agents first, review PRs, glance at running work.

**Flows as composable units**: Steps chain into flows via simple Python definitions. The DAG execution with fork/synthesize is powerful without being overengineered.

**File-based configuration**: Steps, flows, and goals are markdown/Python files—versionable, shareable, reviewable. The system doesn't fight git.

**Stimulus abstraction**: once/loop/watch/cron cleanly models "when should this agent run?" without complex trigger configuration.
