# Agent-First Concerto

Reimagine Concerto around agents instead of worktrees. Every workspace is agent-owned.

## What to build

1. **Rename voice → goal** throughout codebase (CLI, config, UI, docs)
2. **Transform Concerto** from worktree manager to agent launcher
3. **Hide manual worktrees** — only show agent-owned workspaces

## The Shift

| Current | New |
|---------|-----|
| Worktree sidebar | Agent sidebar |
| Voice selector | Goal selector |
| Manual worktree management | Agent-owned worktrees only |
| Agents are secondary | Agents are primary |

## Terminology Change

Rename `voice` → `goal` everywhere:

| Current | New |
|---------|-----|
| `.lf/voices/*.md` | `.lf/goals/*.md` |
| `--voice architect` | `--goal architect` |
| `voice:` in config | `goal:` in config |
| `VoiceService` | `GoalService` |
| Agent.voice | Agent.goal |

The feature works identically — goal files shape agent perspective across all steps.

## User Flow

1. Select **goal** — from `.lf/goals/*.md` (optional)
2. Select **flow** — ship, polish, review (defines the pipeline)
3. Select **area** — path to focus on (e.g., `src/auth/`)
4. Select **stimulus** — once, loop, watch, cron
5. Click **Start** → creates agent, agent creates worktree, runs

## Agent Sidebar

Replaces worktree sidebar. Shows only agent-owned worktrees.

Each agent row displays:
- Goal name (if set)
- Area
- Status indicator (running/idle/waiting/error)
- Flow name
- Iteration count

When selected, detail panel shows:
- Live output (streaming)
- Settings (goal, flow, area, stimulus)
- Iteration history
- PR list from this agent

## Agent Detail Panel

Combines current concepts:
- **Live output** — full-size streaming output (like LoopLiveOutput)
- **Dashboard** — goal, settings, iteration count
- **Worktree info** — commits, changed files, PR status

Tabbed or scrollable sections TBD.

## What Gets Hidden

- Manual worktrees (not agent-owned) — not shown in Concerto
- Step-based prompt launcher — replaced by agent launcher

## v0 Scope

- Rename voice → goal
- Agent sidebar replaces worktree sidebar
- Agent launcher (flow, area, goal, stimulus selectors)
- Agent detail panel (output + dashboard)
- No interactive mode (agents are headless)

---

## Data Structures

### Rename: Voice → Goal

```python
# src/loopflow/lf/goals.py (was voices.py)
@dataclass
class Goal:
    name: str
    content: str

def load_goal(repo: Path | None, goal_name: str) -> Goal | None: ...
def list_goals(repo: Path | None) -> list[str]: ...
def format_goal_section(goal_names: list[str] | None, repo_root: Path) -> str | None: ...
```

```swift
// LoopflowCore/Models/Goal.swift (was Voice.swift)
public struct Goal: Sendable, Identifiable, Hashable {
    public let id: String
    public let name: String
    public let content: String
    public let path: URL
}

// LoopflowCore/Services/GoalService.swift (was VoiceService.swift)
public struct GoalService {
    public func loadGoals(from repoURL: URL) -> [Goal]
}
```

### Agent model (unchanged structure, renamed field)

```swift
public struct Agent: Sendable, Identifiable, Hashable {
    public let id: String
    public let flow: String
    public let goal: [String]      // was: voice
    public let area: [String]
    public let repo: String
    public var stimulus: Stimulus
    public var status: AgentStatus
    public var iteration: Int
    // ... rest unchanged
}
```

### Database schema change

```sql
-- Rename column in agents table
ALTER TABLE agents RENAME COLUMN voice TO goal;

-- Rename column in runs table
ALTER TABLE runs RENAME COLUMN voice TO goal;
```

### Config change

```yaml
# .lf/config.yaml
goal: architect          # was: voice
```

---

## Key Functions

### Python rename map

| Current | New |
|---------|-----|
| `voices.py` | `goals.py` |
| `Voice` | `Goal` |
| `VoiceNotFoundError` | `GoalNotFoundError` |
| `load_voice()` | `load_goal()` |
| `list_voices()` | `list_goals()` |
| `voice_exists()` | `goal_exists()` |
| `resolve_voices()` | `resolve_goals()` |
| `render_voices()` | `render_goals()` |
| `parse_voice_arg()` | `parse_goal_arg()` |
| `format_voice_section()` | `format_goal_section()` |

### Swift rename map

| Current | New |
|---------|-----|
| `Voice.swift` | `Goal.swift` |
| `VoiceService.swift` | `GoalService.swift` |
| `Agent.voice` | `Agent.goal` |
| `FlowRun.voice` | `FlowRun.goal` |
| `loadVoices()` | `loadGoals()` |
| `appState.voices` | `appState.goals` |

### CLI flag rename

```
--voice NAME  →  --goal NAME
-v NAME       →  -g NAME
```

---

## UI Changes

### AgentSidebar (replaces WorktreeSidebar)

```swift
struct AgentSidebar: View {
    @Bindable var appState: AppState

    var body: some View {
        VStack {
            header           // "Agents" + lfd connection + new agent button
            agentList        // ForEach agent → AgentRow
        }
    }
}
```

**Removed:**
- Worktree list
- "Workspaces" section
- New Worktree button/sheet

**Kept:**
- lfd connection indicator
- Agent list (promoted from bottom section)

### AgentLauncher (replaces PromptLauncher)

```swift
struct AgentLauncher: View {
    @Bindable var appState: AppState

    // Selectors
    @State private var selectedFlow: Flow?
    @State private var selectedGoal: Goal?
    @State private var areaPath: String = "."
    @State private var stimulus: Stimulus.Kind = .once

    var body: some View {
        VStack {
            // Flow picker
            Picker("Flow", selection: $selectedFlow) {
                ForEach(appState.flows) { flow in
                    Text(flow.name).tag(flow)
                }
            }

            // Goal picker (optional)
            Picker("Goal", selection: $selectedGoal) {
                Text("None").tag(nil as Goal?)
                ForEach(appState.goals) { goal in
                    Text(goal.displayName).tag(goal)
                }
            }

            // Area text field
            TextField("Area", text: $areaPath)

            // Stimulus picker
            Picker("Stimulus", selection: $stimulus) {
                Text("Once").tag(Stimulus.Kind.once)
                Text("Loop").tag(Stimulus.Kind.loop)
                Text("Watch").tag(Stimulus.Kind.watch)
                Text("Cron").tag(Stimulus.Kind.cron)
            }

            // Start button
            Button("Start") {
                startAgent()
            }
        }
    }
}
```

### AgentDetailPanel (new, replaces WorktreeDetailPanel for selected agent)

```swift
struct AgentDetailPanel: View {
    let agent: Agent
    let liveOutput: [OutputLine]
    let flowRuns: [FlowRun]

    var body: some View {
        VStack {
            // Header: goal, flow, area, status
            AgentHeader(agent: agent)

            // Live output (always visible when running)
            if agent.status == .running {
                OutputPanel(lines: liveOutput)
            }

            // Tabs or sections
            TabView {
                // Dashboard: settings, iteration count
                AgentDashboard(agent: agent)

                // History: past runs
                AgentHistory(runs: flowRuns)

                // PRs created by this agent
                AgentPRList(runs: flowRuns.filter { $0.prUrl != nil })
            }
        }
    }
}
```

### AgentRow (existing, minor updates)

```swift
struct AgentRow: View {
    let agent: Agent
    // ... existing implementation

    // Add context menu
    .contextMenu {
        Button("Start") { onStart() }
        Button("Stop") { onStop() }
        Divider()
        Button("View PRs") { onViewPRs() }
        Button("Land") { onLand() }
        Divider()
        Button("Remove", role: .destructive) { onRemove() }
    }
}
```

---

## File Changes Summary

### Python (rename voice → goal)

| File | Change |
|------|--------|
| `src/loopflow/lf/voices.py` | Rename to `goals.py`, rename all symbols |
| `src/loopflow/lf/config.py` | `voice:` → `goal:` |
| `src/loopflow/lfd/models.py` | `voice` → `goal` |
| `src/loopflow/lfd/agent.py` | `voice` → `goal` |
| `src/loopflow/lfd/cli.py` | `--voice` → `--goal` |
| `src/loopflow/lf/step.py` | Import and usage |
| `src/loopflow/lf/context.py` | Import and usage |
| `src/loopflow/lf/flow.py` | Import and usage |
| `tests/test_voices.py` | Rename to `test_goals.py` |
| `templates/voices/` | Rename to `templates/goals/` |

### Swift (rename + UI overhaul)

| File | Change |
|------|--------|
| `Models/Voice.swift` | Rename to `Goal.swift` |
| `Services/VoiceService.swift` | Rename to `GoalService.swift` |
| `Models/Agent.swift` | `voice` → `goal` |
| `Models/FlowRun.swift` | `voice` → `goal` |
| `Services/AgentService.swift` | Column name in SQL |
| `Views/WorktreeSidebar.swift` | Replace with `AgentSidebar.swift` |
| `Views/PromptLauncher.swift` | Replace with `AgentLauncher.swift` |
| `Views/WorktreeDetailPanel.swift` | Replace with `AgentDetailPanel.swift` |
| `AppState.swift` | Remove worktree selection, add goal loading |

### Docs

| File | Change |
|------|--------|
| `docs/config.md` | `voice:` → `goal:` |
| `docs/lf.md` | `--voice` → `--goal` |
| `docs/lfd.md` | `--voice` → `--goal` |
| `docs/agents.md` | `--voice` → `--goal` |
| `docs/index.md` | Voice → Goal |
| `README.md` | Voice → Goal |

---

## Constraints

- **Database migration required** — rename columns in SQLite
- **Config migration not required** — old `voice:` key can be aliased or just documented as deprecated
- **Backwards-compatible CLI** — could keep `--voice` as hidden alias, or just break it

---

## Done When

```bash
# CLI works with new flag
lf review --goal architect

# Config uses new key
cat .lf/config.yaml
# goal: architect

# Goal files in new location
ls .lf/goals/
# architect.md  designer.md

# Concerto shows only agents
# (no manual worktrees visible)

# Can create agent from Concerto
# - Select flow, area, goal, stimulus
# - Click Start
# - Agent appears in sidebar
```

UI verification:
- Open Concerto
- See agent sidebar (no worktrees section)
- Click "+" to create new agent
- Select flow: ship, area: src/, goal: architect, stimulus: once
- Click Start
- See agent appear in sidebar with status "running"
- Click agent to see detail panel with live output

