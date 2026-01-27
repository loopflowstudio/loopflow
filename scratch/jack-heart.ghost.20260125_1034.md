# Embedded Interactive Sessions

Launch `lf design` (and other interactive steps) directly in Concerto's embedded Ghostty terminal instead of requiring an external terminal.

## Status: In Progress

Core implementation complete:
- [x] Wave creation (worktree creation is future work, not MVP)
- [x] InteractiveSession data structure in SessionState
- [x] InteractiveSessionView wrapping GhosttyTerminalView
- [x] FlowPicker with Interactive toggle
- [x] WaveDetailPanel switches to session mode when interactive session active
- [x] End button kills process and clears session state
- [x] GhosttyManager callback when terminal closes

Not yet implemented:
- [ ] Wave creation creates worktree immediately (current: worktree created on first run)
- [ ] Token summary bar in session view

## What was built

When a user selects a wave and clicks "Run" with Interactive toggle ON, the session runs inside Concerto's embedded terminal—not in Warp or another external app. The user interacts with Claude Code directly in the Concerto window.

## How it works

1. User selects a wave in the sidebar
2. User clicks "Run" with Interactive toggle ON in FlowPicker
3. Concerto shows embedded terminal running `lf <step>` in the wave's worktree
4. User interacts with Claude Code in the embedded terminal
5. User clicks "End" or process exits naturally

## State management

Implemented as three focused state objects (per concerto-simplification.md):

```swift
// RepoState: Primary data - waves, flows, directions, config
@Observable final class RepoState {
    var waves: [Wave]
    var selectedWave: Wave?
    // ...
}

// SessionState: Session tracking - running steps and their output
@Observable final class SessionState {
    var activeSessions: [String: ActiveSession] = [:]
    var interactiveSession: InteractiveSession?
    // ...
}

// LauncherState: Context assembly for prompt launching
@Observable final class LauncherState {
    var selectedPrompt: PromptCard?
    var selectedDirections: [Direction] = []
    // ...
}
```

## InteractiveSession data structure

```swift
// Swift - minimal
struct InteractiveSession: Identifiable {
    let id = UUID().uuidString
    let waveId: String
    let step: String
    let worktreePath: String
    let startedAt = Date()
}
```

## InteractiveSessionView

Wraps GhosttyTerminalView with session header:

```
┌─────────────────────────────────────────┐
│ ● swift-falcon  design [interactive]    │
│                              [End]      │
├─────────────────────────────────────────┤
│                                         │
│  $ lf design                            │
│  ...                                    │
│                                         │
└─────────────────────────────────────────┘
```

- Green status indicator when running
- Wave name and step name displayed
- Interactive badge
- End button kills process and clears session state

## FlowPicker modifications

Added "Interactive" toggle:

```swift
Toggle(isOn: $isInteractive) {
    Text("Interactive")
}
```

When Interactive toggle is ON:
- Run button calls `launchInteractiveSession()` on SessionState
- WaveDetailPanel detects active session and shows InteractiveSessionView

## WaveDetailPanel: Session Mode

WaveDetailPanel checks `sessionState.interactiveSession`:
- If session exists for this wave → show InteractiveSessionView
- Otherwise → show normal config view

## Terminal ownership

When terminal closes (process exits), GhosttyManager's `onSessionClosed` callback triggers:

```swift
GhosttyManager.shared.onSessionClosed = {
    Task { @MainActor in
        sessionState.endInteractiveSession()
    }
}
```

This clears the session state and returns to config mode.

## Constraints

**One session at a time:** MVP supports one interactive session. `sessionState.interactiveSession` is singular.

**Closing Concerto during session:** Kills the process. Interactive sessions need the terminal visible.

## Not in scope (future)

- Detach to external terminal
- Multiple concurrent interactive sessions
- Session resume after Concerto restart
- Token summary bar showing context breakdown
