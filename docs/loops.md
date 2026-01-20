---
layout: default
title: Loops & lfd
---

# Loops & lfd

`lfd` is the loopflow daemon. It runs agent loops in the background, continuously working on goals you define.

![loops demo](loops-demo.gif)

## Concepts

**Loop**: A long-running agent that works toward a goal. Each iteration picks a task, runs a pipeline (design → implement → polish), and creates a PR.

**Goal**: A markdown file in `.lf/goals/` describing what the agent should accomplish. Goals define the agent's focus, quality bar, and iteration strategy.

**Iteration**: One cycle of work. The agent picks a task, executes it, creates a PR, then starts the next iteration.

## Starting the Daemon

The daemon must be running for loops, schedules, and subscriptions to work.

```bash
# One-time setup: install as a launchd service (auto-starts on login)
lfd install

# Or run manually in foreground (for debugging)
lfd serve
```

After `lfd install`, the daemon starts automatically when you log in. Check it's running:

```bash
lfd status    # shows daemon status and all loops
```

## Quick Start

```bash
# Start a loop on a goal
lfd loop product-engineer

# Check status
lfd status

# See created PRs
lfd prs product-engineer
```

## Loop Types

### Continuous Loop

Runs iterations until the PR limit is reached, then waits for reviews.

```bash
lfd loop product-engineer              # continuous loop
lfd loop product-engineer --limit 3    # max 3 outstanding PRs
lfd loop product-engineer --merge-mode land  # auto-land to main
```

### Flow (One-Shot)

Runs exactly one iteration, useful for specific tasks.

```bash
lfd flow designer                      # single iteration
lfd flow designer -p spec.md           # with project file context
lfd flow designer -v                   # with clipboard content
```

## Goals

Goals live in `.lf/goals/` and describe what the agent should do. List available goals:

```bash
lfd list-goals
```

## Options

### Area Override

Focus the agent on specific paths:

```bash
lfd loop product-engineer -a src/api/
lfd loop product-engineer -a "src/api/,tests/api/"
```

### Merge Mode

Control how PRs land:

```bash
lfd loop product-engineer --merge-mode pr    # accumulate, human reviews (default)
lfd loop product-engineer --merge-mode land  # auto-land after each iteration
```

### PR Limit

Control how many PRs can be outstanding:

```bash
lfd loop product-engineer --limit 5    # max 5 unreviewed PRs
```

When the limit is reached, the loop pauses until PRs are merged or closed.

## Branching Model

Each loop gets its own `<goal>-main` branch:

```
main
  └── product-engineer-main  (accumulates iteration work)
        ├── product-engineer/001
        ├── product-engineer/002
        └── product-engineer/003
```

Iteration branches merge to `<goal>-main` automatically. You review and land `<goal>-main` → `main` when ready.

## See Also

[`lfd` reference](lfd.md) · [Triggers](triggers.md) · [Configuration](config.md)
