---
layout: default
title: Getting Started
---

# Getting Started

Loopflow assembles context and prompts for AI coding agents. It gathers your repo docs, diff, and files—then passes everything to Claude, Codex, or Gemini.

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

Tasks are markdown files in `.claude/commands/` or `.lf/`:

```markdown
# .claude/commands/review.md

Review the diff on this branch. Fix any issues.

## What to look for
- Bugs and edge cases
- Style guide violations
- Missing tests
```

Run by name:

```bash
lf review
```

## Context Flags

| Flag | Description |
|------|-------------|
| `-x FILE` | Add a file to context |
| `-v` | Paste clipboard content |
| `--diff` | Include raw `git diff` output |
| `--no-docs` | Skip repo docs |
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

- [`lf` command reference](lf.md) — all flags and options
- [Built-in tasks](builtins.md) — debug, design, implement, polish, review
- [`lfops` commands](lfops.md) — pr, land, commit, init, install
- [Configuration](config.md) — `.lf/config.yaml` options
- [Patterns](patterns.md) — workflows and recipes
