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

## Worktrees

Loopflow works best with git worktrees. Each feature gets its own directory.

```bash
wt switch --create my-feature    # create worktree
lf implement: add auth           # agents work here
lfops pr                         # open PR
lfops land                       # merge and cleanup
```

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

- [Getting Started](getting-started.md) — install, run the demo, write your first task
- [`lf` command reference](lf.md) — all flags and options
- [Built-in tasks](builtins.md) — debug, design, implement, polish, review
- [`lfops` commands](lfops.md) — pr, land, commit, init, install
- [File Storage](storage.md) — where things live and why
- [Configuration](config.md) — `.lf/config.yaml` options
- [Patterns](patterns.md) — workflows and recipes
- [Philosophy](vision.md) — the loopflow mindset
