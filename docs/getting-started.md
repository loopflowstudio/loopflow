---
layout: default
title: Getting Started
---

# Getting Started

## Install

```bash
pip install loopflow
lfops install    # installs Claude Code, worktrunk
```

## Try It

Clone the demo repo and fix a bug:

```bash
git clone https://github.com/loopflowstudio/loopflow-demos
cd loopflow-demos/calculator
python -m pytest test_calc.py    # see the bug
# copy the error to your clipboard
lf debug -v                       # -v pastes clipboard, fixes the bug
```

## How It Works

1. **`lf` gathers context** — repo docs (README, STYLE), files changed on your branch, anything you pass with `-x`
2. **Adds your prompt** — from a task file or inline after `:`
3. **Passes everything to the agent** — Claude Code, Codex, or Gemini CLI

```bash
lf review                     # run .claude/commands/review.md
lf implement: add auth        # pass arguments after colon
lf : "fix the typo"           # inline prompt, no task file
```

## Write Your Own Tasks

Tasks are markdown files in `.claude/commands/`:

```bash
lf review    # runs .claude/commands/review.md
```

## Context Flags

| Flag | Description |
|------|-------------|
| `-x FILE` | Add a file to context |
| `-v` | Paste clipboard content |
| `--diff` | Include raw `git diff` output |
| `--no-lfdocs` | Skip repo docs |
| `-i` | Run interactively (can interrupt) |
| `-a` | Run in auto mode (default) |

## Ship Your Work

```bash
lfops pr      # create PR (CI runs automatically)
lfops land    # submit to merge queue
```

## Worktrees

Loopflow works best with git worktrees. Each feature gets its own directory:

```bash
wt switch --create my-feature    # create worktree
lf implement: add auth           # agents work here
lfops pr                         # open PR (CI runs)
lfops land                       # submit to merge queue
wt remove my-feature             # cleanup after merge
```

## Next Steps

[Built-in Tasks](builtins.md) · [Patterns](patterns.md) · [`lf` reference](lf.md)
