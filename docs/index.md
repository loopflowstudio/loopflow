---
layout: default
title: Home
---

# Loopflow

Arrange agents to code in harmony.

Reusable prompts. Composable workflows.

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
lf implement && lf polish && lf review  # chain tasks
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

Tasks are markdown files in `.claude/commands/`:

```bash
lf review                     # run .claude/commands/review.md
lf implement: add auth        # pass arguments after colon
lf : "fix the typo"           # inline prompt, no file
```

## Worktrees

Loopflow works best with git worktrees. Each feature gets its own directory.

```bash
wt switch --create my-feature    # create worktree
lf implement: add auth           # agents work here
lfops pr                         # open PR
lfops land                       # merge and cleanup
```

## Structured Context

```bash
lf implement -c    # see what the agent sees
```

```
Tokens: 22,932

files          17,625 ███████████████
summary         4,016 ███
task              634 ▏
```

Docs, summaries, branch files—assembled automatically. [Configure what's included →](config.md)

## Multi-Model

Same task, different models:

```bash
lf review -m codex              # use Codex
lf implement -m gemini          # use Gemini
```

## Works With

Loopflow plays nicely with tools in the ecosystem:

- **[worktrunk](https://github.com/loopflowstudio/worktrunk)** — git worktree management
- **[superpowers](https://github.com/obra/superpowers)** — skill library for AI agents

Use your prompts alongside theirs. Any skill, any agent.

---

*Craft and throughput. Not either/or.*

## Next Steps

[Getting Started](getting-started.md) · [Built-in Tasks](builtins.md) · [Patterns](patterns.md) · [Configuration](config.md)
