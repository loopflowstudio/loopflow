# Agents-First Concerto

Reimagine Concerto around a unified agent model. Everything is an agent—autonomous loops, scheduled tasks, and manual interactive sessions.

## What to build

A single agent list replaces the split "Workspaces" / "Loops" sidebar. Creating a new agent is the primary action. Manual work is an agent with `stimulus: manual`.

## User quotes

> "I don't want to understand how it works. I just want it to work."
> "I want the Ruby on Rails of frameworks."

## Core concept

**Everything is an agent.** An agent is area × goal × flow × stimulus.

| What users see | What it actually is |
|----------------|---------------------|
| "Ship auth feature" | agent: area=src/auth, flow=ship, stimulus=loop |
| "Quick fix" | agent: area=., flow=debug, stimulus=manual |
| "Design something" | agent: area=., flow=design, stimulus=manual (interactive) |
| "Nightly polish" | agent: area=., flow=polish, stimulus=cron |

Users don't see worktrees, branches, or triggers. They see agents and their status.

---

## Data structures

### Agent model changes

```swift
// LoopflowCore/Models/Agent.swift

public struct Agent: Sendable, Identifiable, Hashable {
    public let id: String
    public var name: String                    // NEW: user-visible name
    public let flow: String
    public let goal: [String]
    public let area: [String]
    public let repo: String

    public var stimulus: Stimulus
    public var status: AgentStatus
    public var iteration: Int

    // Hidden implementation details
    public var worktreePath: String?           // RENAMED from mainBranch
    public var branch: String?                 // NEW: git branch (auto-generated)

    public var prLimit: Int
    public var mergeMode: MergeMode
    public var pid: Int?
    public var createdAt: Date

    // Derived display properties
    public var displayName: String {
        name.isEmpty ? generateNameFromInput() : name
    }
}
```

### AppState changes

```swift
// Concerto/AppState.swift

@Observable
final class AppState {
    // PRIMARY selection is now agent
    var selectedAgent: Agent?

    // Worktrees still exist but are implementation details
    var worktrees: [Worktree] = []  // kept for internal use

    // Remove: var selectedWorktree: Worktree?

    // Computed: worktree for selected agent
    var selectedWorktree: Worktree? {
        guard let agent = selectedAgent,
              let path = agent.worktreePath else { return nil }
        return worktrees.first { $0.path == path }
    }
}
```

---

## UI structure

### Sidebar: Unified agent list

Replace `WorktreeSidebar` sections with single agent list.

```
┌─────────────────────────────────┐
│ Agents                      + ● │  ← "+" creates agent, dot = lfd status
├─────────────────────────────────┤
│ ● Ship auth feature        ship │  ← running (green)
│   src/auth • iter 3             │
│                                 │
│ ◐ API refactor             ship │  ← waiting (yellow, PR limit)
│   src/api • 2 PRs pending       │
│                                 │
│ ○ Quick fix               debug │  ← idle manual (gray)
│   . • ready                     │
│                                 │
│ ◷ Nightly polish         polish │  ← scheduled (clock)
│   . • 9am daily                 │
└─────────────────────────────────┘
```

Status indicators:
- ● Running (green)
- ◐ Waiting (yellow) — PR limit, rate limit
- ○ Idle (gray) — manual agents, ready to activate
- ◷ Scheduled (clock) — cron agents
- ✓ Completed (checkmark)
- ✗ Error (red)

**No "Workspaces" section.** Orphan worktrees (created via CLI) are hidden from UI.

### Main area: Agent detail

```
┌──────────────────────────────────────────────────────────────┐
│ Ship auth feature                                          ⋮ │
│ src/auth • ship • loop                                       │
├──────────────────────────────────────────────────────────────┤
│ [Warp] [Cursor] [Land PR]                       [Stop Agent] │
├──────────────────────────────────────────────────────────────┤
│ ▼ Current Run (iteration 3)                                  │
│   ● implement ─ ○ polish ─ ○ review                          │
│   Running for 2m 34s                                         │
│                                                              │
│ ▼ Live Output                                                │
│   → Reading src/auth/login.py                                │
│   → Writing src/auth/oauth.py                                │
│                                                              │
│ ▼ History                                                    │
│   Iteration 2: ✓ PR #47 merged                               │
│   Iteration 1: ✓ PR #45 merged                               │
├──────────────────────────────────────────────────────────────┤
│ ▼ Changed Files (3)                                          │
│   src/auth/login.py                  +42 -12                 │
│   src/auth/oauth.py                  +187 (new)              │
│   tests/test_auth.py                 +56 -3                  │
└──────────────────────────────────────────────────────────────┘
```

For **manual agents** (idle), main area shows the **flow picker**:

```
┌──────────────────────────────────────────────────────────────┐
│ Quick fix                                                  ⋮ │
│ . • manual • idle                                            │
├──────────────────────────────────────────────────────────────┤
│ [Warp] [Cursor]                                              │
├──────────────────────────────────────────────────────────────┤
│ Run a flow                                                   │
│ ┌────────────────────────────────────┐                       │
│ │ What should it do?                 │                       │
│ └────────────────────────────────────┘                       │
│                                                              │
│ Flow: [ship ▾]     [Run Once]  [Keep Running ▾]              │
│                                                              │
│ "Keep Running" options:                                      │
│   • Loop (continuous)                                        │
│   • Watch src/ (on change)                                   │
│   • Schedule (cron)                                          │
├──────────────────────────────────────────────────────────────┤
│ ▼ Changed Files (0)                                          │
│   No changes yet                                             │
└──────────────────────────────────────────────────────────────┘
```

This lets any manual agent become a loop/watch/cron agent. The agent persists; its behavior evolves.

### New Agent Sheet

Replace `NewWorktreeSheet` with agent-focused creation.

```
┌──────────────────────────────────────────────────────────────┐
│ New Agent                                                    │
│                                                              │
│ What do you want to work on?                                 │
│ ┌──────────────────────────────────────────────────────────┐ │
│ │ Add OAuth login                                          │ │
│ └──────────────────────────────────────────────────────────┘ │
│                                                              │
│ Name     [                    ]  ← optional, auto-generates  │
│ Area     [. ▾ _______________]   ← typeahead                 │
│                                                              │
│                              [Cancel]  [Create Agent]        │
└──────────────────────────────────────────────────────────────┘
```

**Name**: Optional. If empty, uses `NameGenerator` (adjective-noun like "swift-falcon").

**Area typeahead**: Shows "." plus repo folders. Filters as you type. Like task selector UI.

**Creation flow:**
1. Fill in description (optional), name (optional), area
2. Click "Create Agent"
3. Agent created as `stimulus: manual`, `status: idle`
4. Detail panel shows FlowPicker
5. User chooses: run a flow once, or make it a loop/watch/cron

This is Rails-like: create the resource first, configure behavior after. New agents are "ready to work" - you decide what to run via FlowPicker.

---

## Key functions

```swift
// AgentSidebar.swift (replaces WorktreeSidebar)

struct AgentSidebar: View {
    @Bindable var appState: AppState

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            header           // "Agents" + "+" button + lfd indicator
            agentList        // Unified list of all agents
        }
    }
}

// NewAgentSheet.swift

struct NewAgentSheet: View {
    @State private var description: String = ""
    @State private var name: String = ""           // optional
    @State private var areaSearchText: String = "."
    @State private var isAreaSearchFocused: Bool = false

    var filteredAreas: [String] {
        // "." plus repo folders, filtered by search text
    }

    func createAgent() async throws {
        let agentName = name.isEmpty ? NameGenerator.generate() : name
        // 1. Create agent via lfd with stimulus: manual
        // 2. Agent appears in sidebar as idle
        // 3. User uses FlowPicker in detail panel to run flows
    }
}

// AgentDetailPanel.swift (replaces WorktreeDetailPanel)

struct AgentDetailPanel: View {
    let agent: Agent
    @Bindable var appState: AppState

    var body: some View {
        VStack {
            agentHeader      // name, area, stimulus badge
            actionButtons    // Warp, Cursor, Land (if PR), Stop (if running)

            if agent.status == .idle {
                // Show flow picker for idle agents
                FlowPicker(agent: agent, appState: appState)
            } else {
                // Show run progress for active agents
                runProgress
                liveOutput
                history
            }

            changedFiles
        }
    }
}

// FlowPicker.swift - run flows or change stimulus

struct FlowPicker: View {
    let agent: Agent
    @Bindable var appState: AppState

    @State private var inputText: String = ""
    @State private var selectedFlow: String = "ship"

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Run a flow")
                .font(.caption)
                .foregroundStyle(.secondary)

            // Optional description/args input
            TextField("What should it do?", text: $inputText)

            HStack {
                // Flow picker
                Picker("Flow", selection: $selectedFlow) {
                    ForEach(appState.flows) { flow in
                        Text(flow.name).tag(flow.name)
                    }
                }

                // Run once
                Button("Run Once") {
                    runFlow(stimulus: .once)
                }

                // Keep running menu
                Menu("Keep Running") {
                    Button("Loop (continuous)") {
                        runFlow(stimulus: .loop)
                    }
                    Button("Watch \(agent.areaDisplay) (on change)") {
                        runFlow(stimulus: .watch)
                    }
                    Button("Schedule...") {
                        showSchedulePicker()
                    }
                }
            }
        }
    }

    func runFlow(stimulus: Stimulus.Kind) {
        // Update agent's stimulus and start running
    }
}
```

---

## Constraints

1. **Worktrees are hidden.** Users never see branch names, worktree paths, or git concepts unless they dig into settings or use CLI.

2. **Flows are the vocabulary.** Users learn `ship`, `debug`, `polish`, `design`. Not `stimulus`, `iteration`, `worktree`.

3. **Convention over configuration.** Area defaults to `.`. Flow defaults to `ship`. Goal defaults to none. Model uses config default.

4. **Orphan worktrees stay hidden.** Worktrees created via `lfops wt create` without agents don't appear in UI. Use CLI to manage them.

5. **lfd required for full functionality.** Without daemon, show "Connect lfd" button. Manual agents can work without lfd (just opens terminal).

---

## UI changes summary

| Current | New |
|---------|-----|
| `WorktreeSidebar` | `AgentSidebar` |
| "Workspaces" section | Removed |
| "Loops" section | Renamed to just "Agents" (only section) |
| `WorktreeRow` | `AgentRow` (already exists, enhance) |
| `NewWorktreeSheet` | `NewAgentSheet` |
| `WorktreeDetailPanel` | `AgentDetailPanel` |
| `selectedWorktree` primary | `selectedAgent` primary |

---

## Done when

```bash
# Create and run manually
1. Open Concerto
2. Click "+" to create agent
3. Type "fix the login bug", leave name empty
4. Click "Create Agent"
5. Agent "swift-falcon" appears in sidebar (idle)
6. Detail panel shows FlowPicker
7. Click [Warp] to open terminal, work on fix
8. Changed files appear in detail panel

# Create and run flow once
1. Click "+" → create agent "auth-feature" in src/auth
2. In FlowPicker, select flow "ship"
3. Click [Run Once]
4. Agent status → running, live output streams
5. Flow completes, agent status → idle
6. Changed files and PR link shown

# Turn manual into loop
1. Select existing idle agent
2. In FlowPicker, select flow "ship"
3. Click [Keep Running] → Loop
4. Agent stimulus changes to loop, starts running
5. Continues until stopped or PR limit hit
```

UI verification checklist:
- [ ] Sidebar shows only "Agents" (no "Workspaces" section)
- [ ] "+" button opens NewAgentSheet
- [ ] NewAgentSheet has optional name, area typeahead
- [ ] New agents start as manual/idle
- [ ] Agent list shows all stimulus types (manual, loop, watch, cron)
- [ ] Selecting agent shows detail panel
- [ ] Idle agents show FlowPicker
- [ ] FlowPicker can run once or keep running (loop/watch/cron)
- [ ] Running agents show live output
- [ ] Changed files come from agent's worktree
- [ ] Orphan worktrees not visible

---

## Decisions

1. **Name**: Optional field in NewAgentSheet. If empty, use `NameGenerator` (adjective-noun).

2. **Area picker**: Typeahead like task selector. Shows "." plus folders, filters as you type.

3. **Flow picker in detail panel**: Any agent can run flows or change stimulus.
   - Manual agent detail shows flow picker: "Run flow: [ship ▾] [Run]"
   - Can add/change stimulus: "Keep running as: [loop ▾] [watch ▾] [cron ▾]"
   - Agent is the persistent thing; what it does can change over time.
