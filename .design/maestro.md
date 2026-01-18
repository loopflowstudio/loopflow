# Maestro: The Podium

## What to build

Rebuild Maestro as the place you conduct from — launch tasks, watch progress, ship when ready. Interactive sessions happen in your terminal; Maestro is where you see everything and direct the work.

**Design principle:** Play nice with existing tools. Don't compete with terminals.

---

## UI Layout

```
┌─────────────────────────────────────────────────────────────────┐
│  Loopflow                                        [repo-name ▾]  │
├──────────────┬──────────────────────────────────────────────────┤
│              │  feature-auth                    [PR #42] [Land] │
│  Worktrees   │──────────────────────────────────────────────────│
│              │                                                  │
│  ● main      │  Output                                          │
│  ◐ feature-a │  ────────────────────────────────────────────    │
│  ○ feature-b │  → Read: src/auth.py                             │
│              │  → Edit: src/auth.py (lines 24-31)               │
│  [+ New]     │  → Bash: pytest tests/                           │
│              │  ✓ Tests passed                                  │
│              │                                                  │
│              │──────────────────────────────────────────────────│
│              │  Launcher                                        │
│              │  [implement ▾] [args...                        ] │
│              │  [Run Auto] [Run Interactive]                    │
└──────────────┴──────────────────────────────────────────────────┘

Legend: ● running  ◐ has changes  ○ clean
```

---

## Core screens

**Worktree List (sidebar)**
- Status badges: running, dirty, clean
- Last task, PR state, branch age
- Click to select → main panel shows detail

**Worktree Detail (main panel)**
- Header: branch name, PR link, action buttons (Land, Open in Terminal)
- Output area: streaming text for running tasks OR recent activity
- Launcher: collapsed by default, expands for task selector + args

**Launcher (expandable)**
- Task dropdown + args input
- Context toggles (docs, files, diff, clipboard)
- **Run Auto** → streams output in main panel
- **Run Interactive** → opens external terminal

---

## What changes

| Current | New |
|---------|-----|
| Embedded terminal for all runs | External terminal for interactive only |
| Launch button is primary action | Worktree detail is primary view |
| Terminal-like output panel | Activity feed / streaming log |

---

## Run modes

**Auto tasks:**
1. Spawn `lf <task>` via lfd
2. Stream output to main panel (line-by-line, not PTY)
3. Show status: running → completed/error

**Interactive tasks:**
1. Read terminal from config
2. Open terminal at worktree path
3. Execute `lf <task> -i`
4. Maestro shows "Running interactively in Warp"

---

## Terminal config

Require explicit config in `.lf/config.yaml`:

```yaml
terminal: warp  # or ghostty, iterm, terminal
```

No auto-detection. Error if not configured.

---

## Data model

```swift
struct Worktree {
    let path: URL
    let branch: String
    var status: WorktreeStatus  // clean, dirty, ahead, behind
    var lastTask: TaskRun?
    var pr: PullRequest?
}

struct TaskRun {
    let id: UUID
    let task: String
    let args: String?
    let startedAt: Date
    var endedAt: Date?
    var status: RunStatus  // running, completed, error
    var output: [String]
}
```

---

## Key functions

```swift
// Daemon connection
func connectToLfd() async throws -> LfdConnection
func subscribe(to events: [String]) async  // ["session.*", "output.line"]

// Worktree operations
func loadWorktrees(for repo: URL) async -> [Worktree]

// Task execution
func runAuto(task: String, args: String?, worktree: Worktree) async
func runInteractive(task: String, args: String?, worktree: Worktree) throws

// Terminal
func openTerminal(at path: URL, command: String?) throws
```

---

## Constraints

- No embedded terminal
- Requires lfd daemon for live streaming
- Interactive sessions fully external
- Auto sessions stream line-by-line via lfd socket

---

## Done when

```
1. Open Maestro with a repo that has multiple worktrees
2. Sidebar shows worktrees with status badges
3. Click worktree → main panel shows branch, PR status, output area
4. Run auto task → output streams line-by-line in main panel
5. Run interactive task → external terminal opens
6. No terminal emulator component anywhere
```
