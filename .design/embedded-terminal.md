# Embedded Terminal

## What to build

In-app terminal view that shows Claude Code output directly in Maestro's ResultsPanel, eliminating the context switch to external Terminal/Warp.

## Data structures

```swift
// New: Wraps SwiftTerm for SwiftUI
struct EmbeddedTerminalView: NSViewRepresentable {
    let process: Process?
    @Binding var isRunning: Bool

    func makeNSView(context: Context) -> LocalProcessTerminalView { ... }
}

// Existing: Extended to track embedded terminal state
class TaskRunner: ObservableObject {
    @Published var terminalView: LocalProcessTerminalView?
    @Published var useEmbeddedTerminal: Bool  // user preference
}
```

## Key functions

```swift
// Launch Claude Code with PTY, render in SwiftTerm view
func runTaskEmbedded(command: String, in terminal: LocalProcessTerminalView)

// Existing TerminalLauncher gains a mode switch
func launchTask(_ command: String, embedded: Bool) -> TaskHandle
```

## UI changes

**ResultsPanel.swift**:
- When task is running with embedded mode: show `EmbeddedTerminalView` instead of "Running..." spinner
- Terminal view fills the results area, scrollable
- Keep existing "Running {task}..." header with elapsed time

**PromptLauncher.swift** (Options section):
- Add toggle: "Show output in app" (default: true for auto mode)
- When off, falls back to external terminal launch

**Mockup**:
```
┌─────────────────────────────────────────┐
│ Running implement...          00:45  ⏹  │
├─────────────────────────────────────────┤
│ → Reading src/config.py                 │
│ → Analyzing codebase structure          │
│ → Writing src/auth.py                   │
│                                         │
│ def authenticate(user, password):       │
│     """Validate credentials."""         │
│     ...                                 │
│                                         │
│ █                                       │
└─────────────────────────────────────────┘
```

## Constraints

- **SwiftTerm via SPM** — Add `https://github.com/migueldeicaza/SwiftTerm` to Package.swift
- **Auto mode only initially** — Interactive mode still launches external terminal (needs keyboard input handling)
- **PTY management** — SwiftTerm's `LocalProcessTerminalView` handles this, but we need proper cleanup on task cancel
- **No competing with terminals** — This is output display, not a general terminal. Don't add tabs, profiles, etc.

## Done when

1. `./dev swift` builds with SwiftTerm dependency
2. Running a task in auto mode shows output in ResultsPanel
3. Toggle in Options switches between embedded and external terminal
4. Task completion shows in-app (no need to check external terminal)
5. Cancel button (⏹) kills the process and clears terminal

## Open questions

1. **Terminal font** — Match Maestro's monospace or use SwiftTerm default?
2. **Scrollback** — How much history to keep? SwiftTerm default is 10k lines.
3. **Copy/paste** — Should users be able to select and copy terminal output?
