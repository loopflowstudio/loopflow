# Embedded Interactive Sessions

Launch `lf design` (and other interactive steps) directly in Concerto's embedded Ghostty terminal instead of requiring an external terminal.

## What to build

When a user creates an agent and clicks "Run Design", the session runs inside Concerto's embedded terminal—not in Warp or another external app. The user interacts with Claude Code directly in the Concerto window.

## How it works

1. Agent creation creates a worktree (new behavior)
2. User clicks "Run" with Interactive toggle ON
3. Concerto shows embedded terminal running `lf design` in the worktree

That's it. The `lf` CLI handles context assembly. No new daemon endpoints needed.

## Key change: Agent creation creates worktree

Currently: Agent is created with no worktree. Worktree is created when the agent first runs.

New: `POST /agents` creates the worktree immediately. The agent always has a `worktreePath`.

```python
# agent.py: create_agent()
def create_agent(...) -> Agent:
    # ... existing logic ...

    # Create worktree for this agent
    worktree_path = create_worktree(repo, agent.name)
    agent.worktree = worktree_path

    save_agent(agent)
    return agent
```

## Swift: Launch in embedded terminal

```swift
// When user clicks Run with Interactive ON:
func launchInteractiveSession(agent: Agent, step: String) {
    guard let worktreePath = agent.worktreePath else { return }

    appState.activeSession = InteractiveSession(
        agentId: agent.id,
        step: step,
        worktreePath: worktreePath
    )
}

// InteractiveSessionView shows:
GhosttyTerminalView(
    workingDirectory: session.worktreePath,
    command: "lf \(session.step)"
)
```

## Data structures

```swift
// Swift side - minimal
struct InteractiveSession {
    let agentId: String
    let step: String
    let worktreePath: String
    let startedAt: Date
}
```

## UI changes

### FlowPicker modifications

Add "Interactive" toggle to existing FlowPicker:

```
┌─────────────────────────────────────────┐
│ Try a different flow                    │
│                                         │
│ [design ▼]     [Interactive ✓]  [Run]   │
│                                         │
│ Single step: design                     │
└─────────────────────────────────────────┘
```

When Interactive toggle is ON:
- Run button calls `launchInteractiveSession()` (local, no API call)
- Transitions AgentDetailPanel to "session mode"

### AgentDetailPanel: Session Mode

When an interactive session starts, the detail panel switches to session mode:

```swift
enum DetailPanelMode {
    case config    // Current: shows config, stimulus, flow picker
    case session   // New: shows embedded terminal
}
```

Session mode layout:

```
┌─────────────────────────────────────────┐
│ swift-falcon  design [interactive]      │
│                              [End] [↗]  │
├─────────────────────────────────────────┤
│ Tokens: 12,847                          │
│ files 6,892  diff_files 4,721           │
├─────────────────────────────────────────┤
│                                         │
│  > lf design                            │
│                                         │
│  ━━━ design ━━━                         │
│  Tokens: 12,847                         │
│                                         │
│  files          6,892 █████             │
│    STYLE.md     2,854 ██                │
│                                         │
│  > What would you like to design?       │
│  █                                      │
│                                         │
│                                         │
└─────────────────────────────────────────┘
```

- Header: agent name, step name, mode badge
- `[End]` button: stop session (kills process, returns to config mode)
- `[↗]` button: detach to external terminal (future: not MVP)
- Token summary bar: collapsed view of context breakdown
- Terminal: full remaining height, GhosttyTerminalView

### New View: InteractiveSessionView

Wraps GhosttyTerminalView with session header:

```
┌─────────────────────────────────────────┐
│ swift-falcon  design [interactive] [End]│
├─────────────────────────────────────────┤
│                                         │
│  $ lf design                            │
│  ━━━ design ━━━                         │
│  Tokens: 12,847                         │
│  ...                                    │
│                                         │
└─────────────────────────────────────────┘
```

### State management

```swift
// AppState
@Published var activeSession: InteractiveSession?
```

AgentDetailPanel checks `appState.activeSession`:
- If session exists for this agent → show InteractiveSessionView
- Otherwise → show normal config view

## Constraints

**Terminal ownership:** When terminal closes, need to detect session end and clear `activeSession`. GhosttyManager's `close_surface_cb` should notify Concerto.

**One session at a time:** MVP supports one interactive session. `appState.activeSession` is singular.

## Done when

Manual test from Concerto:

1. Open a repo in Concerto
2. Create new agent → verify worktree is created immediately
3. In FlowPicker, select "design" step, toggle "Interactive" ON
4. Click "Run"
5. Verify: panel switches to session mode with embedded terminal
6. Verify: terminal runs `lf design`, shows token summary, Claude Code starts
7. Type in terminal, verify Claude Code responds
8. Click "End" button → verify returns to config mode

Verification checklist:
- [ ] Agent creation creates worktree
- [ ] Terminal renders with Loopflow burgundy theme
- [ ] Keyboard input works (typing, Ctrl+C, etc.)
- [ ] End button kills process and clears session state

## Open questions

1. **Closing Concerto during session:** Kill the process? Current thinking: yes, interactive sessions need the terminal visible.

## Not in scope (future)

- Detach to external terminal
- Multiple concurrent interactive sessions
- Session resume after Concerto restart
