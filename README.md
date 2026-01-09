# Loopflow

Run LLM coding tasks from reusable prompt files.

macOS only. Supports Claude Code and OpenAI Codex via configuration.

## Install

```bash
pip install loopflow
lf meta install    # installs Claude Code via npm
```

## Why Worktrees?

Loopflow is designed for running background agents while you work on something else. That means isolated branches - you can't have an agent committing to the branch you're actively editing.

The workflow: create a worktree, run tasks there, merge when ready. You can have multiple features in flight at once.

## Quick Start

```bash
lf wt create my-feature       # branch + worktree, opens IDEs
cd ../loopflow.my-feature     # worktrees are sibling directories

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

```bash
lf wt create auth             # ../loopflow.auth/, opens Warp + Cursor
lf wt list                    # show all worktrees
lf wt clean                   # remove merged branches
```

## Session Tracking

Track running tasks across multiple terminals with the maestro daemon:

```bash
lf maestro start              # start tracking daemon
lf status                     # show running sessions

# In another terminal
lf implement -p               # batch task registers automatically

# Check from anywhere
lf status                     # see all running sessions
```

When maestro is running, tasks automatically register, and you'll get macOS notifications when batch tasks complete.

## Configuration

```yaml
# .lf/config.yaml
model: claude     # Model to use (claude or codex)
push: true        # auto-push after commits
pr: false         # open PR after pipelines

ide:
  warp: true
  cursor: true
```

## Options

| Option | Description |
|--------|-------------|
| `-p, --print` | Batch mode (non-interactive) |
| `-x, --context` | Add context files |
| `-w, --worktree` | Create worktree and run task there |
| `-c, --copy` | Copy prompt to clipboard, show token breakdown |
| `-m, --model` | Choose model (claude, codex) |
| `--parallel` | Run with multiple models in parallel |

## Commands

| Command | Description |
|---------|-------------|
| `lf <task>` | Run a task from `.lf/` |
| `lf <pipeline>` | Run a pipeline |
| `lf : "prompt"` | Inline prompt |
| `lf wt create/list/clean` | Worktree management |
| `lf wt compare <a> <b>` | Compare two worktree implementations |
| `lf pr create` | Open GitHub PR |
| `lf pr land [-a]` | Squash-merge to main |
| `lf meta init` | Initialize repo with prompts and config |
| `lf meta install` | Install Claude Code |
| `lf meta doctor` | Check dependencies |
| `lf maestro start` | Start session tracking daemon |
| `lf maestro stop` | Stop session tracking daemon |
| `lf status` | Show running sessions |
