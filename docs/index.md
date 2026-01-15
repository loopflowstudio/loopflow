---
layout: default
title: Home
---

# Loopflow

Run LLM coding agents from reusable prompt files.

Write a task once, run it on any branch:

```bash
lf review          # run .lf/review.lf
lf ship            # pipeline: implement → review → test → commit → PR
```

![Loopflow demo](demo.gif)

## What it is

Loopflow is a workflow for running AI coding agents like Claude Code and Codex. You write prompts as files, chain them into pipelines, and run them across isolated worktrees.

- **Tasks** are prompt files in `.lf/`
- **Pipelines** chain tasks together
- **Worktrees** keep parallel work isolated

If you're experimenting with AI coding tools, this is one way to structure it.

## Install

```bash
# With uv (recommended)
uv tool install loopflow

# Or pip
pip install loopflow

# Install Claude Code + worktrunk if needed
lf ops install
```

## Quick start

Initialize a repo with example tasks:

```bash
cd your-repo
lf ops init
```

This creates `.lf/` with starter prompts and config.

Run a task:

```bash
lf review                # review the current branch
lf implement: add auth   # inline args after the colon
lf : "fix the typo"      # one-off inline prompt
```

Run a pipeline:

```bash
lf ship                  # runs: implement → review → test → commit
```

## Worktrees

Loopflow works best with git worktrees. Each feature gets its own directory, so agents don't conflict with your active work.

```bash
wt switch --create my-feature --execute pwd
cd ../yourrepo.my-feature

lf ship                  # agents work here
# meanwhile, you work in the main repo
```

When done:

```bash
lf ops pr create             # open PR from worktree
lf ops land                  # local merge via worktrunk
wt remove my-feature     # remove worktree + branch
```

## Multi-model

Run with different models:

```bash
lf review -m codex       # use Codex instead of Claude
lf implement --parallel claude,codex  # race them
```

Compare results with `git diff` or your editor.

## Next steps

- [Maestro](maestro.md) - visual interface for loopflow
- [Configuration](config.md) - all options and settings
- [Patterns](patterns.md) - workflows and recipes
- [Daemon (lfd)](lfd.md) - background service and agents
- [API Reference](api.md) - socket protocol for integrations
- [Philosophy](vision.md) - why loopflow exists
