# Loopflow

Arrange agents to code in harmony.

```bash
lf debug -v        # paste error, watch it fix
lf design          # write spec interactively
lf ship            # pipeline: implement → polish → review → PR
```

Store prompts in git. Chain them into pipelines. Run them across isolated worktrees while you work on something else.

macOS. Works with Claude Code, Codex CLI, and Gemini CLI.

## Install

```bash
pip install loopflow
lfops install    # installs Claude Code, Codex, Gemini CLI, worktrunk
```

## Why Worktrees?

Loopflow is designed for running background agents while you work on something else. That means isolated branches - you can't have an agent committing to the branch you're actively editing.

The workflow: create a worktree, run tasks there, merge when ready. You can have multiple features in flight at once.

## Quick Start

**Debug an error:**
```bash
# Run something, get an error, copy it
lf debug -v    # -v pastes clipboard, fixes the bug
```

**Build a feature:**
```bash
wt switch --create my-feature
lf design: add user authentication    # interactive design
lf implement && lf polish && lf review   # autonomous pipeline
lfops pr                               # open PR
```

Try the [demo repo](https://github.com/loopflowstudio/loopflow-demos) for a hands-on example.

## Tasks

Tasks are markdown files in `.claude/commands/` or `.lf/`. Here's an example:

```markdown
# .claude/commands/review.md

Review the diff on the current branch against `main` and fix any issues found.

The deliverable is the fixes themselves, not a written review.

## What to look for

- Style guide violations (read STYLE.md)
- Bugs, logic errors, edge cases
- Unnecessary complexity
- Missing tests
```

Run tasks by name:

```bash
lf review                     # run .claude/commands/review.md
lf review -x src/utils.py     # add context files
lf : "fix the typo"           # inline prompt, no task file
```

All `.md` files at repo root (README, STYLE, etc.) are included as context automatically.

## Pipelines

Chain tasks in `.lf/config.yaml`:

```yaml
pipelines:
  ship:
    tasks: [implement, review, test, commit]
    pr: true    # open PR when done
```

```bash
lf ship    # runs each task, auto-commits between steps
```

## Worktrees

Loopflow delegates worktree management to worktrunk. Use `wt` directly:

```bash
wt list                       # show all worktrees
wt switch --create auth       # create or switch to a worktree
wt remove auth                # remove worktree + branch
```

## Session Tracking

Track running tasks across multiple terminals:

```bash
lfops status                  # show running sessions

# In another terminal
lf implement                  # auto mode task registers automatically

# Check from anywhere
lfops status                  # see all running sessions
```

Sessions write to SQLite in auto mode.

## Background Agents

The `lfd` daemon orchestrates background agents with triggers:

```bash
lfd install                       # install daemon (auto-starts at login)
lfd new my-agent                  # create agent definition
lfd list                          # list agents and status
lfd start my-agent                # manually start an agent
lfd stop my-agent                 # stop a running agent
```

Agents are defined as markdown files in `~/.lf/agents/` with YAML frontmatter.

## Configuration

```yaml
# .lf/config.yaml
agent_model: claude:opus     # Model: claude, codex, gemini (or backend:variant)
push: true        # auto-push after commits
pr: false         # open PR after pipelines

# Tasks that default to interactive mode (default is auto)
interactive:
  - design
  - iterate

ide:
  warp: true
  cursor: true
```

## Run Modes

By default, tasks run in **auto mode**: non-interactive with streaming output. This is ideal for most coding tasks and background execution. All runs append logs under `~/.lf/logs/<worktree>/`.

Use `-i` to run interactively (full chat, can interrupt) or configure per-task defaults:

```bash
lf implement           # auto mode (default)
lf design              # interactive (from config)
lf implement -i        # force interactive
lf design -a           # force auto
lf implement &         # background (shell handles it)
```

## Voices

Voices are reusable personas (system prompts) that shape how the agent responds:

```bash
# Create a voice
mkdir -p .lf/voices
echo "Be concise. One sentence where possible." > .lf/voices/concise.md

# Use it
lf review --voice concise
lf implement --voice architect,concise  # multiple voices
```

Configure a default voice in `.lf/config.yaml`:

```yaml
voice: architect
```

Or per-task in frontmatter:

```yaml
---
voice: concise
---
```

Priority: CLI > frontmatter > config > none.

## Options

| Option | Description |
|--------|-------------|
| `-i, --interactive` | Run in interactive mode (override default) |
| `-a, --auto` | Run in auto mode (override default) |
| `-x, --context` | Add context files |
| `-w, --worktree` | Create worktree and run task there |
| `-c, --copy` | Copy prompt to clipboard, show token breakdown |
| `-v, --paste` | Include clipboard content in prompt |
| `-m, --model` | Choose model (backend or backend:variant) |
| `--voice` | Voice(s) to use (comma-separated) |
| `--parallel` | Run with multiple models in parallel |
| `--race` | Race models, judge the winner |

## Commands

| Command | Description |
|---------|-------------|
| `lf <task>` | Run a task from `.claude/commands/` or `.lf/` |
| `lf <pipeline>` | Run a pipeline |
| `lf : "prompt"` | Inline prompt |
| `wt <subcommand>` | Worktree management (worktrunk) |
| `lfwt list` | Show all worktrees with status |
| `lfwt diff <branch>` | Show diff for a worktree |
| `lfwt compare a b` | Compare two worktree implementations |
| `lfops pr` | Create/update PR, open in browser |
| `lfops land` | Squash-merge to main |
| `lfops commit [-p]` | Generate commit message and commit |
| `lfops init` | Initialize repo with prompts and config |
| `lfops install` | Install Claude Code, Codex, Gemini CLI |
| `lfops doctor` | Check dependencies |
| `lfops status` | Show running sessions |
| `lfd install` | Install and start agent daemon |
| `lfd list` | List agents and their status |
| `lfd start <name>` | Start an agent |
| `lfd stop <name>` | Stop a running agent |
| `lfwork list` | List work items |
| `lfwork add "title"` | Add work item |
| `lfwork next` | Show next available work |
