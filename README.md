# Loopflow

## Usage

```bash
lf review
lf implement: add auth
lf ship
```

Run LLM coding tasks from reusable prompt files.

macOS only. Supports Claude Code and OpenAI Codex via configuration.

## Install

```bash
pip install loopflow
lf ops meta install    # installs Claude Code + worktrunk
```

## Why Worktrees?

Loopflow is designed for running background agents while you work on something else. That means isolated branches - you can't have an agent committing to the branch you're actively editing.

The workflow: create a worktree, run tasks there, merge when ready. You can have multiple features in flight at once.

## Quick Start

```bash
wt switch --create my-feature --execute pwd
cd ../loopflow.my-feature

lf design                     # interactive: figure out what to build
lf ship                       # batch: implement, review, test, commit, open PR
```

`lf design` runs `.lf/design.lf`. `lf ship` runs the `ship` pipeline from `.lf/config.yaml`.

## Tasks

Tasks are prompt files in `.lf/`. Here's an example:

```markdown
# .lf/review.lf

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
lf review                     # run .lf/review.lf
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

Track running tasks across multiple terminals (maestro is optional):

```bash
lf ops maestro start              # optional web UI (tails logs)
lf ops status                     # show running sessions (reads SQLite)

# In another terminal
lf implement                  # auto mode task registers automatically

# Check from anywhere
lf ops status                     # see all running sessions
```

Sessions write to SQLite in auto mode; the maestro UI reads the same database.

## Configuration

```yaml
# .lf/config.yaml
agent_model: claude:opus     # Model to use (backend or backend:variant)
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
| `--parallel` | Run with multiple models in parallel |

## Commands

| Command | Description |
|---------|-------------|
| `lf <task>` | Run a task from `.lf/` |
| `lf <pipeline>` | Run a pipeline |
| `lf : "prompt"` | Inline prompt |
| `lf ops compare a b` | Compare two worktree implementations |
| `wt <subcommand>` | Worktree management (worktrunk) |
| `lf ops pr create` | Open GitHub PR |
| `lf ops pr land [-a]` | Squash-merge to main |
| `lf ops land [--no-pr] [--force] [--base]` | Land locally via worktrunk |
| `lf ops meta init` | Initialize repo with prompts and config |
| `lf ops meta install` | Install Claude Code |
| `lf ops meta doctor` | Check dependencies |
| `lf ops maestro start` | Start session tracking daemon |
| `lf ops maestro stop` | Stop session tracking daemon |
| `lf ops status` | Show running sessions |
