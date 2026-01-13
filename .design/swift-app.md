# Loopflow Maestro (macOS App)

A native macOS app for managing worktrees and launching LLM coding sessions.

## What to build

A Swift app that "opens" a repo (like Cursor or Xcode) and provides two core experiences:

1. **Worktree Launcher** — GUI for `wt`. See all worktrees, jump to terminal/IDE.
2. **Prompt Launcher** — GUI for `lf`. Browse prompts, customize context, launch sessions.

> "Loopflow Maestro 'opens' a repo just the same as you do with Cursor or Xcode"
> "I prefer having .lf/config.yaml be 'automatic config' rather than manually opening a code-workspace"
> "A nice text box entry point that is a substitute for opening a Claude Code or ChatGPT web session"

## Design principles

**80% Notion, 20% Stripe.** Clean, minimal, confident typography. Generous whitespace. No chrome clutter. Actions feel immediate. Stripe's precision in details—hover states, transitions, data display.

**Learn from Conductor:** Task-centric view, progress visibility, clean session management.

**Automatic configuration.** Open a folder → app discovers `.lf/config.yaml` → ready to go. No workspace files to manage.

---

## Part 1: Worktree Launcher

A GUI wrapper for `wt`. The left sidebar of the app.

```
┌─────────────────────────────────────────────────────────────┐
│  loopflow                                        ⚙️  │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  WORKTREES                                    + New         │
│                                                             │
│  ┌─────────────────────────────────────────────────────┐   │
│  │  main                                    ✓ clean    │   │
│  │  ↳ 2 commits ahead                                  │   │
│  └─────────────────────────────────────────────────────┘   │
│                                                             │
│  ┌─────────────────────────────────────────────────────┐   │
│  │  maestro                              ● 3 modified   │   │
│  │  ↳ PR #42 · 5 commits ahead                         │   │
│  └─────────────────────────────────────────────────────┘   │
│                                                             │
│  ┌─────────────────────────────────────────────────────┐   │
│  │  swift-app                               ✓ clean    │   │
│  │  ↳ no PR · 12 commits ahead                         │   │
│  └─────────────────────────────────────────────────────┘   │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

### Worktree actions

Click a worktree → context menu or action bar:

- **Open in Terminal** — Opens Warp (or configured terminal) at worktree path
- **Open in Cursor** — Auto-detects `.code-workspace` if present, else opens folder
- **Open in Finder** — Reveal in Finder
- **View PR** — Opens GitHub PR in browser (if exists)
- **Delete** — Runs `wt remove` (with confirmation)

**"+ New" button** — Creates a new worktree:
- Modal with branch name input
- Optional base branch selector (defaults to main)
- Runs `wt switch --create <name>`
- Auto-opens in terminal after creation

**Double-click** — Opens in terminal. (What would Notion do? Open the thing.)

Full CRUD on worktrees—no safer to hide these than the CLI.

### Data model

```swift
struct Worktree: Identifiable {
    let id: String           // branch name
    let path: URL
    let branch: String
    let isDirty: Bool
    let aheadMain: Int
    let behindMain: Int
    let prURL: URL?
    let prNumber: Int?
    let hasCodeWorkspace: Bool  // for Cursor auto-detection
}
```

Read from `wt list --format json --full`. Poll every few seconds or on window focus.

---

## Part 2: Prompt Launcher

The main content area. A GUI for composing and launching `lf` commands.

```
┌─────────────────────────────────────────────────────────────┐
│                                                             │
│  ┌─────────────────────────────────────────────────────┐   │
│  │  What do you want to build?                         │   │
│  │                                                     │   │
│  │  Add dark mode toggle to settings              ⌘↵  │   │
│  └─────────────────────────────────────────────────────┘   │
│                                                             │
│  PROMPTS                                                    │
│                                                             │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐      │
│  │ implement│ │  review  │ │  design  │ │  polish  │      │
│  │          │ │          │ │          │ │          │      │
│  │   auto   │ │   auto   │ │ interact │ │   auto   │      │
│  └──────────┘ └──────────┘ └──────────┘ └──────────┘      │
│                                                             │
│  CONTEXT                                      12,340 tokens │
│                                                             │
│  ☑ Include diff against main                               │
│  ☑ Repository docs (README, STYLE)                         │
│  ☐ Include folder: src/loopflow/cli/                       │
│  ☐ Include folder: tests/                                  │
│                                                             │
│  MODE                                                       │
│                                                             │
│  ◉ Auto (non-interactive, streams output)                  │
│  ○ Interactive (full conversation)                         │
│                                                             │
│                                    [Run in Terminal]  ⌘↵   │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

### Flow

1. **Text entry** — Type what you want to build. This becomes the `{args}` in the prompt.
2. **Select prompt** — Click a prompt card (implement, review, design, etc). Cards show default mode.
3. **Customize context** — Toggle diff, select folders to include. Token count updates live.
4. **Choose mode** — Auto or interactive. Defaults from `.lf/config.yaml`.
5. **Launch** — Opens terminal (Warp), runs `lf <prompt> <args>` with configured options.

### Terminal integration

From `.lf/config.yaml`:

```yaml
terminal: warp      # warp, iterm, terminal, kitty
ide: cursor         # cursor, vscode, zed
workspace: loopflow.code-workspace  # optional, for Cursor/VSCode
```

Launch command:

```swift
func launchInTerminal(command: String, at path: URL) {
    switch config.ide.terminal {
    case "warp":
        // warp://action/new_tab?path=...&command=...
        let url = URL(string: "warp://action/new_tab?path=\(path.path)&command=\(command.urlEncoded)")!
        NSWorkspace.shared.open(url)
    case "iterm":
        // AppleScript or URL scheme
    default:
        // Terminal.app via AppleScript
    }
}
```

### Prompt cards

Read from `.claude/commands/*.md` and show as selectable cards:

```swift
struct PromptCard: Identifiable {
    let id: String           // filename without extension
    let name: String         // e.g., "implement"
    let content: String      // full prompt text
    let defaultMode: RunMode // from config.interactive list
}

enum RunMode {
    case auto
    case interactive
}
```

**No default selection.** We don't know which prompts exist in any given repo. User picks one, or just types in the text box for inline prompt.

### Context picker

Notion-style: tree view with toggles, not a separate file picker modal. Shows repo structure with checkboxes. Expandable folders. Checking a folder includes all contents.

```
☑ src/
  ☑ loopflow/
    ☐ cli/
    ☑ maestro/
    ☐ ...
☐ tests/
☑ README.md
☑ STYLE.md
```

### Token visualization

Live token count updates as user toggles context. Shells out to `lf` for accuracy.

```swift
func estimateTokens(prompt: String, args: String, context: [String]) async -> Int {
    // lf <prompt> <args> -x <context...> -c --json
    let result = try await Process.run("lf", [prompt, args] + context.flatMap { ["-x", $0] } + ["-c", "--json"])
    return parseTokenCount(result.stdout)
}
```

---

## Data structures

```swift
// App state
@Observable
class AppState {
    var currentRepo: URL?
    var config: LoopflowConfig?
    var worktrees: [Worktree] = []
    var prompts: [PromptCard] = []

    // Prompt launcher state
    var selectedPrompt: PromptCard?
    var promptArgs: String = ""
    var includeDiff: Bool = true
    var includeFolders: Set<URL> = []
    var runMode: RunMode = .auto
}

// Config from .lf/config.yaml
struct LoopflowConfig: Codable {
    let agentModel: String?
    let interactive: [String]?  // prompts that default to interactive
    let terminal: String?       // warp, iterm, terminal, kitty
    let ide: String?            // cursor, vscode, zed
    let workspace: String?      // .code-workspace file
    let context: [String]?
    let exclude: [String]?
}
```

---

## File structure (monorepo)

```
loopflow/
├── src/loopflow/              # Python library (existing)
├── pyproject.toml             # Python package (existing)
├── Loopflow/                  # Swift app (new)
│   ├── Loopflow.xcodeproj/
│   ├── Loopflow/
│   │   ├── LoopflowApp.swift
│   │   ├── AppState.swift
│   │   ├── Views/
│   │   │   ├── ContentView.swift
│   │   │   ├── WorktreeSidebar.swift
│   │   │   ├── PromptLauncher.swift
│   │   │   └── PromptCard.swift
│   │   ├── Models/
│   │   │   ├── Worktree.swift
│   │   │   ├── PromptCard.swift
│   │   │   └── Config.swift
│   │   └── Services/
│   │       ├── WorktreeService.swift
│   │       ├── TerminalLauncher.swift
│   │       └── ConfigLoader.swift
│   └── LoopflowTests/
├── .lf/                       # Loopflow config (existing)
└── README.md
```

---

## Constraints

- **macOS 15+ (Sequoia).** Users are sophisticated; demand up-to-dateness. Enables latest SwiftUI, Observable macro.
- **Python CLI is authoritative.** App shells out to `lf` and `wt`, never duplicates logic.
- **SQLite is shared state.** CLI writes to `~/.lf/maestro.db`, app reads it. No daemon needed for tracking—app polls the database. Agents are just processes; they persist when app closes.
- **Terminal-first launching.** Sessions run in user's terminal, not embedded.
- **Direct distribution.** .dmg or Homebrew cask, not App Store. Avoids sandbox restrictions.

> "CLI `lf` commands should be tracked by the Maestro app"

The app doesn't need to launch agents—it just needs to see them. Users can run `lf implement` from any terminal and Maestro shows it.

---

## Done when

1. App opens a repo and loads `.lf/config.yaml`
2. Sidebar shows worktrees from `wt list --format json`
3. Can click worktree → open in Warp (or configured terminal)
4. Can click worktree → open in Cursor (auto-detects workspace)
5. Prompt launcher shows available prompts from `.claude/commands/`
6. Can type args, select prompt, toggle context, see token estimate
7. "Run in Terminal" opens Warp and executes `lf <prompt> <args>`
8. Distributed as .dmg with auto-update (Sparkle)

---

## Out of scope (for now)

- Background agent monitoring (future: Part 3)
- Session output streaming in-app
- Embedded terminal
- App Store distribution
