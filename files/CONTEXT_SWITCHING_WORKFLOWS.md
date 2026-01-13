# Context Switching Workflows for AI Coding Agents

A deep dive into how power users manage notifications, terminal tabs, window layouts, and multi-agent orchestration when running parallel Claude Code / Codex sessions.

---

## The Core Problem: Orchestration Is Now the Developer's Job

From Takafumi Endo (Medium, June 2025):
> "The primary task of the developer shifts from the cognitive load of coding to the cognitive load of orchestration. Keeping track of multiple worktrees, managing the context of several AI agents, and ensuring a coherent integration strategy requires immense discipline."

The old bottleneck was **writing code**. The new bottleneck is **managing agents**.

---

## Part 1: Notification Systems

### Where Notifications Get Registered

Claude Code has multiple notification mechanisms, each with different terminal support:

#### 1. Terminal Bell (Built-in)

```bash
# Enable globally
claude config set --global preferredNotifChannel terminal_bell

# Test if your terminal supports it
echo -e "\a"
```

**Supported:** iTerm2, Ghostty, most Unix terminals
**Not Supported:** Many xterm.js implementations, VS Code terminal (inconsistent)

#### 2. OSC Escape Sequences (Advanced)

From kane.mx (December 2025):
```bash
# OSC 777 format (VSCode, rxvt-unicode)
printf '\033]777;notify;Title;Message\007'

# OSC 9 format (iTerm2, Windows Terminal)
printf '\033]9;Message\007'
```

**Key insight:** VSCode's integrated terminal forwards OSC sequences through SSH tunnels. This means remote Claude Code sessions can trigger local desktop notifications.

#### 3. Hooks System (Most Flexible)

Claude Code hooks fire at specific lifecycle events:

```json
{
  "hooks": {
    "Notification": [
      {
        "matcher": "",
        "hooks": [
          {
            "type": "command",
            "command": "notify-send 'Claude Code' 'Awaiting your input'"
          }
        ]
      }
    ],
    "Stop": [
      {
        "matcher": "",
        "hooks": [
          {
            "type": "command", 
            "command": "osascript -e 'display notification \"Task Done\" with title \"Claude Code\"'"
          }
        ]
      }
    ]
  }
}
```

**Hook Events:**
- `PreToolUse` - Before tool calls (can block)
- `PostToolUse` - After tool calls
- `Notification` - When Claude needs input/permission
- `Stop` - When agent finishes (not on user interrupt)
- `SessionEnd` - When session closes

#### 4. Platform-Specific Solutions

**macOS (terminal-notifier):**
```bash
brew install terminal-notifier
terminal-notifier -title "Claude Code" -message "Task complete" -sound Glass
```

**macOS (native osascript):**
```bash
osascript -e 'display notification "Task completed" with title "Claude Code" sound name "Glass"'
```

**Linux (notify-send):**
```bash
notify-send "Claude Code" "Awaiting your input"
```

**Windows WSL (PowerShell):**
```bash
powershell.exe -c "[System.Media.SystemSounds]::Question.Play()"
```

**iPhone/Apple Watch (Pushcut):**
From Justin Searls:
> "I wanted to be able to walk away from my Mac while it cooked. This led me to cobble together this solution that will ping my iPhone and Apple Watch with a push notification whenever Claude needs my attention."

Uses Pushcut webhooks ($2/month Pro tier) triggered by Claude Code hooks.

### The Multi-Window Problem

From kane.mx:
> "If you're like me, you probably have multiple VSCode Remote SSH windows open to the same server, working on different projects. The basic implementation sends notifications to all terminal devices."

**Solution:** UUID-based terminal mapping using `VSCODE_IPC_HOOK_CLI` environment variable to route notifications to the correct window.

### Community Tools

| Tool | Platform | Method |
|------|----------|--------|
| [claude-code-notifier](https://github.com/hta218/claude-code-notifier) | macOS/Linux | Shell script + hooks |
| terminal-notifier | macOS | CLI tool |
| Pushcut | iOS | Webhook service |
| ntfy.sh | Cross-platform | Self-hosted push notifications |

---

## Part 2: Terminal Tab/Window Management

### The steipete Setup (Power User Baseline)

From steipete (August 2025):
> "Still extremely happy with my Dell UltraSharp U4025QW - 3840x1620 makes 4 Claude instances + Chrome all visible without moving windows."

His approach: **One ultrawide monitor, 4 terminal panes visible simultaneously, no worktrees.**

> "I tried the whole worktree setup, just slows me down... I simply commit to main."

### The Ghostty Philosophy

From Takafumi Endo:
> "Ghostty's GPU-accelerated rendering and native pane management are not luxuries; they are the essential I/O layer that makes orchestrating this complexity feasible without the interface itself becoming a bottleneck. Its minimalist design philosophy is a strategic feature. By eschewing the integrated AI and collaborative clutter of its peers, it reduces cognitive noise."

**Ghostty's strengths for multi-agent work:**
- Native splits (no tmux dependency)
- GPU-accelerated (handles rapid output streams)
- Minimalist (reduces cognitive load)
- Config-file based (reproducible setups)

**The title problem** (from steipete, June 2025):
> "Ghostty only shows the process name in the title. When you have six tabs all saying 'claude', finding the right one becomes a game of terminal roulette."

**His solution:** Custom ZSH configuration that sets terminal titles based on working directory + session ID.

### Terminal Comparison for Multi-Agent Workflows

| Terminal | Splits | Session Persistence | Notifications | Best For |
|----------|--------|---------------------|---------------|----------|
| **Ghostty** | Native | Via config | OSC sequences | Speed + minimalism |
| **iTerm2** | Native | Excellent | Native macOS | tmux integration |
| **Warp** | Native | Built-in | Native + AI | Blocks, collaboration |
| **WezTerm** | Native | Lua scripted | Configurable | Programmable workflows |
| **Alacritty** | None | Requires tmux | Bell only | Raw performance |
| **kitty** | Native | Persistent | Kitty protocol | Graphics/images |

### tmux: The Multiplexer Approach

From blle.co:
```bash
#!/bin/bash
# claude-dev-session.sh
tmux new-session -d -s claude-dev
tmux rename-window -t claude-dev:0 'Claude Code'
tmux new-window -t claude-dev:1 -n 'Tests'
tmux new-window -t claude-dev:2 -n 'Logs'
tmux new-window -t claude-dev:3 -n 'Git'

# Start Claude Code in the first window
tmux send-keys -t claude-dev:0 'claude' Enter

# Set up test watcher in second window
tmux send-keys -t claude-dev:1 'npm run test:watch' Enter

tmux attach-session -t claude-dev
```

**Why tmux + Claude Code:**
- **Session persistence:** Detach, close laptop, reattach later
- **Remote development:** SSH into server, Claude keeps running
- **Pane layouts:** Claude in main, tests in right, logs below

**Workflow from blle.co:**
> "Morning Start: Attach to yesterday's session - Claude's context is preserved. Development: Split panes - Claude in main, tests in right, logs below. End of Day: Leave session running for tomorrow's instant resume."

---

## Part 3: Multi-Agent Orchestration Tools

### Claude Squad (The Leading Solution)

From GitHub (smtg-ai/claude-squad):

**What it does:**
- TUI for managing multiple Claude Code / Aider / Codex instances
- Uses tmux for session isolation
- Uses git worktrees for code isolation
- Single interface to view all agents

**Usage:**
```bash
# Install
curl -fsSL https://raw.githubusercontent.com/smtg-ai/claude-squad/main/install.sh | bash

# Launch
cs

# Create new instance
# (within TUI) press 'n', enter task name

# Toggle between diff and preview
# (within TUI) press 'tab'

# Auto-accept prompts (YOLO mode)
cs --autoyes
```

**Key features:**
- Each agent gets isolated git worktree + tmux session
- Background task completion with `--autoyes`
- Keyboard shortcuts: `tab` (toggle diff/preview), `ctrl-q` (detach), `shift-↑/↓` (scroll diffs)

**Architecture:**
```
┌─────────────────────────────────────────┐
│           Claude Squad TUI              │
├─────────────────────────────────────────┤
│  Instance 1    │  Instance 2    │ ...   │
│  ┌──────────┐  │  ┌──────────┐  │       │
│  │ tmux     │  │  │ tmux     │  │       │
│  │ session  │  │  │ session  │  │       │
│  └──────────┘  │  └──────────┘  │       │
│  ┌──────────┐  │  ┌──────────┐  │       │
│  │ git      │  │  │ git      │  │       │
│  │ worktree │  │  │ worktree │  │       │
│  └──────────┘  │  └──────────┘  │       │
└─────────────────────────────────────────┘
```

### uzi (Proposed, Not Yet Built)

From DEV Community (May 2025):
```bash
# Initialize Claude and Codex instances with same prompt
uzi start --agents claude:3,codex:2 --prompt "Implement feature X"

# List all active agents
uzi ls

# Run command across all worktrees
uzi exec --all -- yarn dev

# Send follow-up to all agents
uzi broadcast -- "Refine the previous response by focusing on Y"

# Checkpoint specific agent
uzi checkpoint --agent claude-1 --message "Implemented initial draft"
```

**Current pain points uzi would solve:**
> "I manually create git worktrees, start a tmux session for each one, run Claude Code in the first pane, paste a prompt, leader+c into a new pane, run yarn dev to get a preview, switch to my browser to review..."

### GitButler (Lifecycle Hooks Approach)

From GitButler blog (July 2025):
> "With Claude Code hooks, you can make Claude tell GitButler when a file is about to be edited... We do a quick commit that stores the prompt used to generate that change."

**The innovation:** Multiple Claude sessions in the **same directory**, but GitButler automatically sorts changes into separate branches based on session ID.

**Result:**
- No worktrees needed
- No merge conflicts with yourself
- One commit per chat round
- One branch per Claude Code session

### Conductor (macOS Native)

From Aitoolnet:
> "Run multiple Claude Code agents in parallel on your Mac. Conductor provides isolated workspaces, automated Git, & full oversight for efficient AI development."

### Gitpod (Cloud Environments)

From Gitpod blog:
> "There are emerging workarounds—tools like Claude Squad uses tmux to split terminals; Vibe Kanban overlays a kanban UI on top of agents; others isolate agents in containers. But they all share the same flaw: they're all just workarounds."

**Gitpod's solution:** Each Claude Code agent runs in its own cloud development environment with dedicated CPU, memory, file system, and git state.

---

## Part 4: Window Grouping Strategies

### The Cursor + Claude Code Hybrid

Many developers run both:
- **Cursor** for visual editing, UI work, quick fixes
- **Claude Code** for deep reasoning, multi-file refactoring, architecture

From Sidetool:
> "Use Cursor to handle real-time edits and debugging, while Claude Code manages architectural design, refactoring plans, or test generation."

### Peacock Extension (Visual Differentiation)

From Medium (August 2025):
> "Multiple windows make it hard to tell which project you are working in. With this extension, you can make it so that each window gets a different colour at the top and bottom."

**Peacock** (VS Code/Cursor extension) lets you assign colors to workspaces:
- Project A = Blue header
- Project B = Green header
- etc.

### Rectangle / Rectangle Pro (macOS Window Management)

**Rectangle** (free, open-source):
- Keyboard shortcuts for window positioning
- Snap to screen edges
- Move between monitors

**Rectangle Pro** ($9.99):
- Custom snap areas
- Application layouts (restore entire workspace)
- Window throw gestures

**Example developer layout:**
```
┌─────────────────┬─────────────────┐
│                 │                 │
│   Cursor/IDE    │   Claude Code   │
│   (left half)   │   (right half)  │
│                 │                 │
├─────────────────┼─────────────────┤
│   Browser       │   Terminal/Logs │
│   (bottom-left) │   (bottom-right)│
└─────────────────┴─────────────────┘
```

### macOS Spaces / Mission Control

Some developers use separate Spaces per project:
- Space 1: Project A (Cursor + Claude Code + Browser)
- Space 2: Project B (different set of windows)
- Swipe between with trackpad

**Limitation:** Can't see both projects simultaneously.

### Ultrawide Monitor Strategy (steipete style)

From steipete:
> "3840x1620 makes 4 Claude instances + Chrome all visible without moving windows."

**Layout:**
```
┌────────┬────────┬────────┬────────┬────────┐
│Claude 1│Claude 2│Claude 3│Claude 4│ Chrome │
│  auth  │  api   │  tests │  docs  │  app   │
└────────┴────────┴────────┴────────┴────────┘
```

No window switching needed—everything visible at once.

---

## Part 5: Workflow Patterns from Power Users

### Pattern 1: The steipete "No Worktrees" Approach

> "I tried the whole worktree setup, just slows me down. If you pick areas of work carefully you can work on multiple areas without much cross-pollination."

**Setup:**
- 4 Ghostty terminals, all in same directory
- All commit to main
- Blast radius thinking: small changes = low risk
- Refactor often to avoid debt

**When this works:**
- Solo developer
- Good mental model of codebase
- Disciplined about scope

### Pattern 2: The Simon Willison "Mixed Fleet" Approach

> "My daily drivers are currently Claude Code (on Sonnet 4.5), Codex CLI (on GPT-5-Codex), and Codex Cloud (for asynchronous tasks, frequently launched from my phone.)"

**Setup:**
- Multiple terminal windows in different directories
- Fresh checkout into /tmp for isolation (not worktrees)
- YOLO mode for trusted contexts
- Codex Cloud for risky/async tasks

### Pattern 3: The Claude Squad Approach

```bash
# Start Claude Squad
cs

# Create instances for different features
# Instance 1: "Implement auth"
# Instance 2: "Write tests"
# Instance 3: "Update docs"

# Each gets isolated worktree + tmux session
# Toggle between them in TUI
# Merge winning implementation
```

### Pattern 4: The iTerm2 + tmux + Worktrees Approach

From DEV Community:
```bash
# Create worktrees
git worktree add ../project-feature-auth feature/auth
git worktree add ../project-feature-api feature/api

# In iTerm2, create panes (Cmd+D horizontal, Cmd+Shift+D vertical)
# Pane 1: cd ../project-feature-auth && claude
# Pane 2: cd ../project-feature-api && claude

# Each Claude session maintains its own context
# Switch panes with Cmd+[ and Cmd+]
```

### Pattern 5: The Architect + Implementers Pattern

From Jesse Vincent (September 2025):
> "Having an architect agent iterate on a plan which is then reviewed and implemented by fresh instances of Claude Code."

**Workflow:**
1. Main Claude session creates detailed plan
2. Plan saved to file
3. Spawn N Claude instances in worktrees
4. Each implementer reads plan, executes assigned portion
5. Review and merge results

---

## Part 6: The Notification + Context Switching Integration

### Ideal Workflow (What People Want)

1. **Start work:** Launch 3-4 Claude agents on different tasks
2. **Background processing:** Agents work autonomously
3. **Smart notifications:** Get pinged only when:
   - Agent needs input/permission
   - Agent completes task
   - Agent encounters error
4. **Quick context switch:** Click notification → jump to correct terminal/window
5. **Review results:** Compare outputs, merge best implementation
6. **Cleanup:** Close completed agents, prune worktrees

### Current Gaps

| Need | Current State | Pain |
|------|---------------|------|
| **Notification routing** | Broadcasts to all terminals | Have to check each one |
| **Task identification** | Terminal title says "claude" | Can't tell which task |
| **Status dashboard** | None built-in | Have to manually check each agent |
| **Completion detection** | Stop hook fires | No centralized view |
| **Quick switch** | Keyboard shortcuts | Need to remember which pane/window |

### What a Good Solution Would Provide

From Feature Request #4963:
> "True Agentic Parallelism: Empowers developers to delegate multiple complex, long-running tasks without blocking their own interactive workflow. Reduced Cognitive Load: Abstracts away the tedious and error-prone mechanics of managing git worktrees, multiple shells, and background processes into a single, clean interface."

**Proposed commands:**
- `/fork "implement feature X"` - Spawn agent in new worktree
- `/tasks` - Show status dashboard
- `/tasks attach <ID>` - Jump to specific agent
- `/tasks merge <ID>` - Merge agent's work, cleanup worktree

---

## Summary: The Context Switching Stack

### For Solo Developers

| Layer | Tool | Purpose |
|-------|------|---------|
| **Window Management** | Rectangle / Rectangle Pro | Snap windows, keyboard shortcuts |
| **Terminal** | Ghostty or iTerm2 | Native splits, GPU rendering |
| **Multiplexer** | tmux (optional) | Session persistence, remote work |
| **Agent Management** | Claude Squad | TUI for multiple agents |
| **Notifications** | Hooks + terminal-notifier | Know when agents need attention |
| **Visual Differentiation** | Peacock (Cursor) / custom titles | Tell windows apart |

### For Teams

| Layer | Tool | Purpose |
|-------|------|---------|
| **Isolation** | Gitpod / Codespaces | Cloud environments per agent |
| **Orchestration** | Custom scripts / CCPM | Coordinate multi-agent work |
| **Notifications** | Slack integration / webhooks | Team-wide visibility |
| **Code Isolation** | Git worktrees | Parallel branches |
| **Merge Strategy** | GitButler / manual | Combine results |

### Key Takeaways

1. **Notifications are solved** (hooks + terminal-notifier + Pushcut), but routing to the correct window is still manual

2. **Tab management requires discipline:** Custom terminal titles, color coding, or tools like Claude Squad

3. **Worktrees vs. same-directory:** Power users are split—steipete says worktrees slow him down, others swear by them

4. **The real gap:** No unified dashboard showing all agents + their status + quick switching

5. **tmux is the glue:** Most multi-agent setups use tmux for session isolation, even if the terminal has native splits

6. **Ultrawide > multiple monitors** for keeping everything visible without window switching

---

## Tools Referenced

| Tool | URL | Purpose |
|------|-----|---------|
| Claude Squad | github.com/smtg-ai/claude-squad | Multi-agent TUI |
| Rectangle | rectangleapp.com | macOS window management |
| Ghostty | ghostty.org | GPU-accelerated terminal |
| tmux | github.com/tmux/tmux | Terminal multiplexer |
| terminal-notifier | github.com/julienXX/terminal-notifier | macOS notifications |
| Peacock | marketplace.visualstudio.com | VS Code color themes |
| GitButler | gitbutler.com | Virtual branches |
| Pushcut | pushcut.io | iOS push notifications |
| Gitpod | gitpod.io | Cloud dev environments |

---

*Research compiled January 2026*
