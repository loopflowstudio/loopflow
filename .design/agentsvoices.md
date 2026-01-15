# Agents & Voices in Maestro

**What to build:** Add a Voices picker to the repo window and a separate Agents window for managing background agents.

## User Priorities (verbatim)

> "Voices are ways to compose additional prompts into tasks. They are optional, you don't need to use one, and you can use multiple. They can affect the style of prose, but also the focus of attention, tone, etc."

> "Agents are a whole new window, because Agents *create* worktrees."

> "Our primary job here is: (1) Identify things to be done later to plug this all together on the backend (2) build out most of the Maestro Swift UX here, working as much as possible but mostly focused around UX and design to start."

---

## Part 1: Voices

Voices live in `.lf/voices/{name}.md` within a worktree. They're composable system prompts that shape agent behavior.

### Data Structures

```swift
// Models/Voice.swift
struct Voice: Identifiable, Hashable {
    let id: String       // filename without .md
    let name: String
    let content: String
    let path: URL
}
```

```swift
// Services/VoiceService.swift
struct VoiceService {
    func loadVoices(from repoURL: URL) -> [Voice]
    // Reads .lf/voices/*.md
}
```

### UI: Voice Picker in PromptLauncher

Add voice selection between the Task selector and main input. Click to open popover with available voices.

```
┌─────────────────────────────────────────────────────────────┐
│  Task   [ implement ▼ ]                                     │
│                                                             │
│  Voice  [ architect ] [ concise ]              (click to add)│
│                                                             │
│  ┌───────────────────────────────────────────────────────┐  │
│  │ What do you want to build?                            │  │
│  │                                                       │  │
│  └───────────────────────────────────────────────────────┘  │
│                                                             │
│  Auto / Interactive            Run ⌘↵                       │
└─────────────────────────────────────────────────────────────┘
```

Selected voices appear as dismissable chips. Clicking the row opens a popover listing available voices from `.lf/voices/`. No creation flow—voices must exist on disk.

### Key Functions

```swift
// In PromptLauncher.swift
@State private var selectedVoices: [Voice] = []

private var voiceSelector: some View { ... }
private func voiceChip(_ voice: Voice) -> some View { ... }
private func voicePickerPopover() -> some View { ... }
```

```swift
// AppState additions
var voices: [Voice] = []
var selectedVoices: [Voice] = []

func refreshVoices() async { ... }
func buildCommand() -> String {
    // Add: --voice architect,concise
}
```

### Starter Voices (3)

Already created in `.lf/voices/`:

**architect.md**
```
Focus on system design and architecture. Consider:
- Data flow between components
- Error handling boundaries
- Extension points for future work
- API surface area (smaller is better)
```

**concise.md**
```
Be concise. One sentence where possible. Skip preamble.
When explaining, use examples over prose. When listing options, bullet points over paragraphs.
```

**thorough.md**
```
Be thorough. Consider edge cases, failure modes, and performance implications.
Add tests for new behavior. Update documentation when changing public APIs.
```

### Backend Work (Deferred)

None—backend already has `voices.py` with `load_voice()` and `parse_voice_arg()`.

---

## Part 2: Agents Window

Agents are global (not repo-bound). They live in `~/.lf/agents/*.md` and create worktrees when run.

### Data Structures

Agent model already exists. Extend it:

```swift
// Models/Agent.swift additions
struct Agent {
    // existing fields...
    let emoji: String          // for branch naming like 🎯/agent-name/001
    let goal: String?          // path to goal doc (relative to repo)
    let mergeStrategy: String  // "pr" or "auto"
    let cron: String?          // cron expression if trigger==cron
}

enum TriggerKind: String {
    case manual
    case mainChanged = "main-changed"
    case loop
    case cron
}
```

### UI: Agent Window

NavigationSplitView like repo window. Sidebar lists agents, main panel shows selected agent's config.

```
┌────────────────────────┬────────────────────────────────────────────────┐
│ AGENTS            [+]  │                                                │
├────────────────────────┤  Goal                                          │
│                        │  ┌────────────────────────────────────────────┐│
│ 🎯 refine-ui      ●    │  │ Improve conversion on the signup flow.     ││
│    loopflow            │  │ Focus on reducing friction...              ││
│                        │  └────────────────────────────────────────────┘│
│ 📊 daily-review   ○    │                                                │
│    myapp               │  Pipeline   [ ship ▼ ]                         │
│                        │                                                │
│ 🔄 continuous     ○    │  Trigger    [ main-changed ▼ ]                 │
│    myapp               │                                                │
│                        │  Context    [ src/components ] [ src/auth ]    │
│                        │                                                │
│                        │  Merge      ◉ Open PR   ○ Auto-merge           │
│                        │                                                │
│                        │  ────────────────────────────────────────────  │
│                        │                                                │
│                        │  Status     ● Running (iteration 3)            │
│                        │  Worktree   loopflow.🎯-refine-ui-003          │
│                        │  Last run   2 min ago                          │
│                        │                                                │
│                        │              [ Stop ]  [ Open Worktree ]       │
│                        │                                                │
└────────────────────────┴────────────────────────────────────────────────┘
```

**Sidebar** - Compact rows: emoji + name + status dot + repo name. Click to select.

**Main Panel** - Two sections:
1. **Config** (top) - Editable fields: Goal, Pipeline, Trigger, Context, Merge strategy
2. **Status** (bottom) - Runtime info: current status, worktree path, last run, actions

### Key Functions

```swift
// Views/AgentWindow.swift
struct AgentWindow: View {
    @State private var agents: [Agent] = []
    @State private var selectedAgent: Agent?
    @State private var showingNewAgent = false

    private let agentService = AgentService()

    var body: some View {
        NavigationSplitView {
            AgentSidebar(agents: agents, selected: $selectedAgent, onAdd: { ... })
        } detail: {
            if let agent = selectedAgent {
                AgentDetailPanel(agent: agent, onSave: { ... }, onStart: { ... }, onStop: { ... })
            } else {
                Text("Select an agent")
            }
        }
    }
}
```

```swift
// Views/AgentSidebar.swift
struct AgentSidebar: View {
    let agents: [Agent]
    @Binding var selected: Agent?
    let onAdd: () -> Void

    private func agentRow(_ agent: Agent) -> some View { ... }
}
```

```swift
// Views/AgentDetailPanel.swift
struct AgentDetailPanel: View {
    let agent: Agent
    let onSave: (Agent) -> Void
    let onStart: () -> Void
    let onStop: () -> Void

    // Editable state (copied from agent on appear)
    @State private var goal: String = ""
    @State private var pipeline: String = ""
    @State private var triggerKind: TriggerKind = .manual
    @State private var cronExpression: String = ""
    @State private var contextPaths: [String] = []
    @State private var mergeStrategy: MergeStrategy = .pr

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 24) {
                configSection
                Divider()
                statusSection
            }
            .padding()
        }
    }

    private var configSection: some View { ... }
    private var statusSection: some View { ... }
    private var triggerPicker: some View { ... }  // Shows cron input when trigger==cron
}
```

```swift
// MaestroApp.swift - add window
WindowGroup(id: "agents") {
    AgentWindow()
}
.windowStyle(.automatic)
.defaultSize(width: 800, height: 600)
```

### Trigger-Specific UI

The trigger picker shows additional fields based on selection:

| Trigger | Additional Fields |
|---------|------------------|
| manual | None |
| main-changed | None |
| loop | None |
| cron | Cron expression text field, grace period stepper |

```swift
private var triggerPicker: some View {
    VStack(alignment: .leading, spacing: 8) {
        Picker("Trigger", selection: $triggerKind) {
            Text("Manual").tag(TriggerKind.manual)
            Text("On main change").tag(TriggerKind.mainChanged)
            Text("Loop").tag(TriggerKind.loop)
            Text("Cron").tag(TriggerKind.cron)
        }

        if triggerKind == .cron {
            TextField("0 9 * * *", text: $cronExpression)
                .textFieldStyle(.roundedBorder)
            Text("Runs daily at 9am")
                .font(.caption)
                .foregroundStyle(.secondary)
        }
    }
}
```

### Menu Bar Access

Add menu item: **Window > Agents** (⌘⇧A) to open the agents window from any context.

### Status Section Actions

| Status | Available Actions |
|--------|------------------|
| Idle | Start |
| Running | Stop, Open Worktree |
| Waiting | Stop |
| Error | Start (retry), View Log |
| Stopped | Start |

When running, show link to open worktree in terminal/IDE (same pattern as repo window).

### Creating a New Agent

Click [+] in sidebar → Sheet with minimal required fields:

```
┌─────────────────────────────────────────────┐
│ New Agent                                   │
│                                             │
│ Name      [ my-agent          ]             │
│ Emoji     [ 🎯 ]  (picker or text)          │
│ Repo      [ ~/src/myapp       ] [Browse]    │
│                                             │
│           [Cancel]        [Create]          │
└─────────────────────────────────────────────┘
```

Creates `~/.lf/agents/my-agent.md` with defaults (manual trigger, ship pipeline). User configures the rest in the main panel after creation.

### Goal Field

The Goal is the agent's purpose—free-form text that gets injected into each pipeline step. It's stored in the agent's markdown file body (after frontmatter).

```markdown
---
emoji: 🎯
repo: ~/src/myapp
pipeline: ship
trigger: main-changed
---

Improve conversion on the signup flow. Focus on:
- Reducing form fields
- Clearer error messages
- Faster validation feedback
```

The Goal textarea in the UI edits this body text. Changes auto-save to the `.md` file (debounced).

### Backend Work (Deferred)

1. **Looping trigger** - `TriggerKind.LOOP` runs indefinitely, generates diff per iteration
2. **Pipeline storage** - where to store agent-specific pipelines (not in worktree, not in main)
3. **Auto-close worktrees** - cleanup after merge
4. **Goal file watching** - re-run when goal doc changes

---

## Part 3: Pipelines

Pipelines connect voices/agents to task execution. Current structure in `lfd/pipelines.py` supports DAGs with parallel steps.

### Where Pipelines Live

- **Repo pipelines**: `.lf/pipelines/{name}.yaml` - for worktree tasks
- **Agent pipelines**: TBD - agents create worktrees, can't edit main

For now, agents reference repo pipelines by name. The pipeline runs in the created worktree.

### UI Consideration (Deferred)

Could show pipelines as a third view or inline in both Voices and Agents views. Defer—current config.yaml pipelines work fine for now.

---

## Constraints

- **Voices are worktree-scoped.** Adding a new voice adds a file to the current worktree's `.lf/voices/`.
- **Agents are global.** They live in `~/.lf/agents/` and can target any repo.
- **Agents create worktrees.** They don't run in existing worktrees—they spawn new ones.
- **Backend already exists.** Don't duplicate `voices.py` or `agents.py` logic in Swift.

---

## Done When

1. **Voices picker appears in PromptLauncher**
   - Load from `.lf/voices/*.md` on repo open
   - Multi-select chips, persist across runs
   - `--voice` flag added to command builder

2. **Agents window opens from menu**
   - Window > Agents (⌘⇧A)
   - Lists agents from `~/.lf/agents/`
   - Start/stop via context menu or buttons
   - Shows status, iteration count, last run

3. **Starter voices created** ✓
   - `architect.md`, `concise.md`, `thorough.md` already in `.lf/voices/`

```bash
# Verification
uv run lf design --voice concise  # should include voice in command
# Agents window should list any agents in ~/.lf/agents/
```
