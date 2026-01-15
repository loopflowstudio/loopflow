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

New top-level window (not inside repo window). Lists all agents with status, actions, history.

```
┌──────────────────────────────────────────────────────────────────────┐
│ Agents                                                    [+] New    │
├──────────────────────────────────────────────────────────────────────┤
│                                                                      │
│  🎯 refine-ui                          ● Running                     │
│     loopflow • ship • main-changed                                   │
│     Iteration 3 • 2 min ago                                          │
│                                                        [Stop] [View] │
│                                                                      │
│  ──────────────────────────────────────────────────────────────────  │
│                                                                      │
│  📊 daily-review                       ○ Idle                        │
│     myapp • review • cron(0 9 * * *)                                 │
│     Last run: 22h ago                                                │
│                                                       [Start] [Edit] │
│                                                                      │
│  ──────────────────────────────────────────────────────────────────  │
│                                                                      │
│  🔄 continuous-test                    ○ Idle                        │
│     myapp • test • loop                                              │
│     Not run yet                                                      │
│                                                       [Start] [Edit] │
│                                                                      │
└──────────────────────────────────────────────────────────────────────┘
```

### Key Functions

```swift
// Views/AgentWindow.swift
struct AgentWindow: View {
    @State private var agents: [Agent] = []
    @State private var showingNewAgent = false

    var body: some View { ... }
    private func agentRow(_ agent: Agent) -> some View { ... }
}
```

```swift
// Views/AgentEditorSheet.swift
struct AgentEditorSheet: View {
    // Create/edit agent definition
    @State private var name: String
    @State private var emoji: String
    @State private var repo: URL?
    @State private var pipeline: String
    @State private var triggerKind: TriggerKind
    @State private var cronExpression: String
    @State private var mergeStrategy: String
    @State private var goal: String
    @State private var prompt: String
}
```

```swift
// MaestroApp.swift - add window
WindowGroup(id: "agents") {
    AgentWindow()
}
.windowStyle(.automatic)
.defaultSize(width: 600, height: 500)
```

### Menu Bar Access

Add menu item: **Window > Agents** (⌘⇧A) to open the agents window from any context.

### Agent Detail / History View

Click "View" on a running/completed agent to see:
- Current worktree (if running)
- Run history with timestamps
- Link to open worktree / view PR

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
