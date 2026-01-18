# Reframe: CLI-First Positioning + Maestro as Dashboard

## What to build

Reposition loopflow as a CLI tool for prompt reuse and workflow composition. Rebuild Maestro as a monitoring dashboard, not a terminal replacement.

---

## Part 1: Documentation Reframe

### Core message

Loopflow is a system for storing prompts in git and chaining them into workflows. That's the product. Background agents and GUI are roadmap items, not the entry point.

**Keep:** "Arrange agents to code in harmony" (poetic, aspirational)
**Add subtitle:** "Reusable prompts. Composable workflows." (concrete, approachable)

The tagline stays. The subtitle grounds it for people who don't know what that means yet.

### What stays (Level 1)

- Prompts as markdown files in `.claude/commands/` or `.lf/`
- `lf <task>`, `lf : "inline"`, context flags
- `lfops pr`, `lfops land`, `lfops commit` — git workflow
- Worktrees via `wt` (worktrunk)
- Manual chaining: `lf implement; lf polish; lfops pr`

### What gets de-emphasized (Level 2 / Beta)

- Pipelines in `.lf/config.yaml` → power feature, not intro material
- `lfd` daemon and background agents
- Session tracking / SQLite → internal detail
- Multi-model racing
- Maestro → "Coming soon" teaser

### New docs structure

**Level 1: Core (main navigation)**
```
docs/
  index.md          # Quick start: install, run a task, ship with lfops
  tasks.md          # Writing and organizing prompts
  workflow.md       # lfops pr, lfops land, lfops commit
  config.md         # .lf/config.yaml reference (trimmed to L1 features)
  patterns.md       # Recipes (debug, design-first, manual chaining)
```

**Level 2: Advanced/Beta (separate section)**
```
docs/advanced/
  pipelines.md      # Declarative task chaining, auto-commit
  agents.md         # lfd daemon, background agents
  multi-model.md    # Racing, parallel execution
  api.md            # Socket protocol for integrations
```

**Coming Soon (teaser)**
```
docs/
  roadmap.md        # Maestro GUI, future vision
```

### The Level 1 workflow story

```bash
# Debug an error
lf debug -v                      # paste error, watch it fix

# Build a feature
wt switch --create my-feature    # worktree for isolation
lf design: add auth              # interactive design
lf implement                     # build it
lf polish                        # run tests, fix issues
lfops pr                         # open PR

# Land it
lfops land                       # squash-merge, cleanup
```

No pipelines. No daemon. Just tasks and git workflow.

### Key quotes to preserve

> "Store prompts in git."

> "Tasks are markdown files."

> "Tight loops. Do one thing, hand off cleanly."

---

## Part 2: Maestro Rebuild

### Philosophy shift

Maestro is a **dashboard for monitoring agents**, not a terminal replacement. Users who want interactive sessions use their preferred terminal. Maestro shows what's happening across worktrees.

**Design principle:** Play nice with existing tools. Don't compete with terminals.

### UI Layout

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

### Core screens

**1. Worktree List (sidebar)**
- All worktrees with status badges (running, dirty, clean)
- Last task, PR state, branch age
- Click to select → main panel shows detail

**2. Worktree Detail (main panel)**
- Header: branch name, PR link if exists, action buttons
- Output area: streaming text for running tasks OR recent activity log
- Launcher: collapsed by default, expands to show task selector + args

**3. Launcher (expandable panel)**
- Task dropdown + args input
- Context toggles (docs, files, diff, clipboard)
- **Run Auto** → streams output in main panel
- **Run Interactive** → opens external terminal

### What changes from current Maestro

| Current | New |
|---------|-----|
| Embedded terminal for all runs | External terminal for interactive only |
| Launch button is primary action | Worktree detail is primary view |
| Terminal-like output panel | Activity feed / streaming log |
| Session history in sidebar | Session history in main panel |

### Interactive prompt behavior

When user clicks "Run Interactive" or runs an interactive task:

1. Determine user's preferred terminal (config or detect)
2. Open terminal at worktree path
3. Execute `lf <task> -i` in that terminal
4. Maestro shows "Running interactively in Warp" with link to focus

No embedded terminal. The terminal app handles the session.

### Auto prompt behavior

When user clicks "Run Auto":

1. Spawn `lf <task>` in background
2. Stream output to main panel (not a PTY, just line-by-line text)
3. Show status: running → completed/error
4. Log appears in activity feed

### Data model

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
    var output: [String]   // lines of output
}

enum RunMode {
    case auto      // stream to Maestro
    case interactive  // open external terminal
}
```

### Key functions (Swift)

```swift
// Daemon connection
func connectToLfd() async throws -> LfdConnection
func subscribe(to events: [String]) async  // ["session.*", "output.line"]

// Worktree operations
func loadWorktrees(for repo: URL) async -> [Worktree]
func refreshWorktree(_ worktree: Worktree) async -> Worktree

// Task execution
func runAuto(task: String, args: String?, worktree: Worktree) async
    // Sends to lfd, subscribes to output.line events

func runInteractive(task: String, args: String?, worktree: Worktree) throws
    // Opens terminal app, executes command

// Terminal launching
func openTerminal(at path: URL, command: String?) throws
    // Reads terminal config, opens appropriate app
```

### Terminal configuration

Require explicit config in `.lf/config.yaml`:

```yaml
terminal: warp  # or ghostty, iterm, terminal
```

No auto-detection. If not configured, show error prompting user to set it.

Supported terminals: Warp, Ghostty, iTerm2, Terminal.app

---

## Constraints

- No embedded terminal in this version
- Maestro requires lfd daemon for live streaming (show prompt to install if not running)
- Interactive sessions are fully external (require `terminal` config)
- Auto sessions stream line-by-line via lfd socket, not PTY
- Keep existing CLI behavior unchanged

---

## Done when

### Docs
```bash
# Main docs focus on tasks + lfops workflow
grep -r "lfd\|daemon" docs/*.md   # only in docs/advanced/
grep -r "pipeline" docs/*.md      # only in docs/advanced/
grep -r "Maestro" docs/*.md       # only in docs/roadmap.md

# README has new subtitle
head -5 README.md | grep "Reusable prompts"

# Quick start shows manual chaining, not pipelines
grep "lf implement" docs/index.md | grep -v "lf ship"
```

### Maestro
```
Manual verification:
1. Open Maestro with a repo that has multiple worktrees
2. Sidebar shows worktrees with status badges
3. Click worktree → main panel shows branch, PR status, output area
4. Run an auto task → output streams line-by-line in main panel
5. Run an interactive task → external terminal opens (Warp/Ghostty/etc)
6. No terminal emulator component visible anywhere
```

---

## Decisions made

- **Tagline:** Keep "Arrange agents to code in harmony" + subtitle "Reusable prompts. Composable workflows."
- **Level 1:** Tasks (`lf`) + git workflow (`lfops`) + worktrees (`wt`)
- **Level 2:** Pipelines, lfd daemon, multi-model, session tracking
- **Coming soon:** Maestro
- **Live output:** Maestro requires lfd daemon for streaming (connects via socket)
- **Terminal:** Require explicit config (`terminal: warp`), no auto-detection magic
