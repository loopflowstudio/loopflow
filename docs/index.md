---
layout: default
title: Home
---

# Loopflow

Run LLM coding agents from reusable prompt files.

```bash
lf review          # run .claude/commands/review.md
lf ship            # pipeline: implement → review → test → commit → PR
```

![Loopflow demo](demo.gif)

## What it is

Loopflow treats prompts as artifacts. They live in your repo, versioned with git, not scattered across chat logs and clipboards.

```
.lf/
├── review.lf      # your code review standards
├── implement.lf   # how you want features built
├── ship.yaml      # pipeline: implement → review → test → commit
└── config.yaml    # settings
```

Each `.lf` file is markdown. `git log` shows prompt history. `git diff` shows what changed. When something works, you can find it again.

## Why this matters

**Prompts accumulate knowledge.** "Include error handling." "Follow our naming conventions." "Check for the bugs we always make." These belong in version control, not your head.

**Structure makes it robust.** Vibing produces slop. A pipeline that reviews before committing doesn't. `lf ship` means "implement, then review, then test, then commit" — every time.

**Tools change fast.** Your prompts should survive. Same task file runs on Claude, Codex, or Gemini. Switch with `-m codex`.

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

- [Maestro](maestro.md) — prefer a GUI? Native Mac app for visual prompt launching
- [Configuration](config.md) — all options for `.lf/config.yaml`
- [Patterns](patterns.md) — real workflows and recipes from production use
- [Philosophy](vision.md) — why loopflow exists, what we're betting on
