---
layout: default
title: Maestro
---

# Maestro

*Coming soon.*

The podium: visual control, real-time feedback. See your agents at work.

Maestro is a native Mac app that wraps the CLI. Point-and-click prompt selection, live streaming output, worktree management in a sidebar. Start with the CLI to learn the system; add Maestro when you're managing multiple agents.

## Getting Started

### Installation

Download the latest release from GitHub releases or build from source:

```bash
cd Maestro
xcodebuild -scheme Maestro -configuration Release
```

### Opening a Repository

1. Launch Maestro
2. Open a repository: **File → Open** or drag a folder onto the dock icon
3. The main window shows the prompt launcher and worktree sidebar

Recent repositories appear on the welcome screen for quick access.

## Prompt Launcher

The prompt launcher is the main interface for running tasks.

### Task Selector

Select a task from the dropdown or type to search. Tasks are loaded from `.claude/commands/` or `.lf/` in your repository. The mode badge shows whether the task defaults to auto or interactive.

### Args Input

The large text field accepts your prompt or arguments. Format: `task: args` or just text for inline prompts.

Examples:
- `implement: add user authentication` — run implement with args
- `review` — run review with no args
- `fix the typo in README` — inline prompt (no task file)

### Context Options

Click **Context** to expand options:

| Toggle | Description |
|--------|-------------|
| Docs | Include `.md` files from repo root |
| Files | Include full content of files changed on this branch |
| Diff | Include raw `git diff` output |
| Clipboard | Include clipboard contents |

**Attached files**: Drag files onto the drop zone or paste with ⌘V. Attached files are passed to the agent as additional context.

### Token Estimation

The token count shows estimated context size. The distribution bar visualizes how context is split between docs, diff, files, and clipboard.

### Running

Click **Run** or press **⌘↵** to launch. If you're on the main branch, Maestro creates a new worktree automatically with a generated name.

## Worktree Sidebar

The left sidebar shows all worktrees for the current repository.

### Status Badges

Each worktree shows a colored badge for the last completed task:

| Badge | Task |
|-------|------|
| Blue | design |
| Green | implement |
| Orange | review |
| Purple | polish |

Click a worktree to select it as the target for the next prompt.

### Actions

Right-click a worktree for actions:

- **Open in Terminal** — Opens the worktree in your configured terminal
- **Open in IDE** — Opens in Cursor (if configured)
- **Create PR** — Runs `lfops pr` to create or update a pull request
- **View PR** — Opens the existing PR in browser
- **Land** — Runs `lfops land` to merge and clean up

### Creating Worktrees

Click **+** or use **⌘N** to create a new worktree. Enter a branch name or leave blank for a generated name.

### Deleting Worktrees

Right-click → **Delete** removes the worktree and its branch. This is equivalent to `wt remove <branch>`.

## Agents Panel

View and manage background agents.

### Viewing Agents

The agents panel shows all agents defined in `~/.lf/agents/`. Each entry displays:

- Agent name and emoji
- Current status (idle, running, waiting, error)
- Iteration count
- Trigger type

### Starting/Stopping

Click **Start** to begin an agent, **Stop** to halt it. Agents run in the background and appear in the session history.

## Session History

Click the clock icon to view session history for the selected worktree. History shows:

- Task name and arguments
- Start/end time
- Status (running, completed, error)
- Model used

Sessions are stored in `~/.lf/lfd.db` and persist across app restarts.

## Keyboard Shortcuts

| Shortcut | Action |
|----------|--------|
| ⌘↵ | Run prompt |
| ⌘N | New worktree |
| ⌘O | Open repository |
| ⌘W | Close window |
| ⌘1 | Focus worktree sidebar |
| ⌘2 | Focus prompt input |

## Configuration

Maestro reads `.lf/config.yaml` from the open repository. Key settings:

```yaml
# Terminal to use for launching tasks
ide:
  warp: true      # Use Warp (default)
  cursor: true    # Open Cursor alongside

# Default run mode
interactive:
  - design        # These tasks default to interactive
```

See [Configuration](../config.md) for all options.
