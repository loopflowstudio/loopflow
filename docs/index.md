---
layout: default
title: Home
---

# Loopflow

Run LLM coding agents from reusable prompt files.

## Quick Debug

Copy an error, run one command, watch it fix.

![Debug demo](debug-demo.gif)

```bash
# Run tests, copy the error to clipboard
lf debug -v
```

The `-v` flag pastes your clipboard. Loopflow reads the stacktrace, finds the bug, fixes it.

## Full Workflow

Design a feature interactively, then let agents implement it.

![Design demo](design-demo.gif)

```bash
wt switch --create add-divide           # create a worktree
lf design: add division to calculator   # interactive design session
lf implement && lf polish && lf review  # autonomous pipeline
```

Each task reads what the previous one wrote. Design produces a spec, implement builds it, polish runs tests, review gives a verdict.

## Install

```bash
pip install loopflow
lfops install    # installs Claude Code, worktrunk
```

## Try It

Clone the demo repo and follow along:

```bash
git clone https://github.com/loopflowstudio/loopflow-demos
cd loopflow-demos/calculator
python -m pytest test_calc.py    # see the bug
# copy error to clipboard
lf debug -v                       # fix it
```

## How It Works

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
lf review                     # run .claude/commands/review.md
lf implement: add auth        # pass arguments after colon
lf : "fix the typo"           # inline prompt, no file
```

## Pipelines

Chain tasks in `.lf/config.yaml`:

```yaml
pipelines:
  ship:
    tasks: [implement, review, polish, commit]
    pr: true
```

```bash
lf ship    # runs each task, commits between steps
```

## Worktrees

Loopflow works best with git worktrees. Each feature gets its own directory.

```bash
wt switch --create my-feature    # create worktree
lf ship                          # agents work here
lfops pr                         # open PR
lfops land                       # merge and cleanup
```

## Multi-Model

Same task, different models:

```bash
lf review -m codex              # use Codex
lf implement --race claude,codex   # race them, pick winner
```

## Next Steps

- [Configuration](config.md) — all options for `.lf/config.yaml`
- [Patterns](patterns.md) — workflows and recipes
- [Maestro](maestro.md) — native Mac app for visual control
- [Daemon](lfd.md) — background agents
