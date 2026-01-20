---
layout: default
title: Background Agents
---

# Background Agents

Define goals, review PRs when you wake. `lfd` runs agent loops in the background.

![loops demo](loops-demo.gif)

## Quick start

```bash
lfd install                      # one-time: install daemon
lfd loop product-engineer        # start a loop
lfd status                       # check progress
lfd prs product-engineer         # see created PRs
```

## Concepts

**Loop**: A long-running agent working toward a goal. Each iteration picks a task, runs a pipeline (design → implement → polish), creates a PR.

**Goal**: A markdown file in `.lf/goals/` describing what the agent should accomplish.

**Iteration**: One cycle of work. Pick task, execute, create PR, repeat.

## Starting the daemon

```bash
lfd install    # installs launchd service, auto-starts on login
lfd status     # verify it's running
```

Or run manually: `lfd serve`

## Continuous loops

```bash
lfd loop product-engineer              # run continuously
lfd loop product-engineer --limit 3    # max 3 outstanding PRs
lfd loop product-engineer --merge-mode land  # auto-land to main
```

When the PR limit is reached, the loop pauses until PRs are merged.

## One-shot flows

Run exactly one iteration:

```bash
lfd flow designer                # single iteration
lfd flow designer -p spec.md     # with project file
lfd flow designer -v             # with clipboard
```

## Triggers

Beyond continuous loops, agents can react to events.

### Subscribe to file changes

```bash
lfd subscribe "src/api/**" api-docs-updater
```

When files matching the pattern change on main, runs one iteration.

```bash
lfd subscribe "src/api/routes/**" openapi-updater
lfd subscribe "schema.graphql,src/resolvers/**" graphql-codegen
```

### Schedule with cron

```bash
lfd schedule "0 9 * * *" daily-triage
```

Schedules have a 24-hour grace period—if your laptop wakes after the scheduled time but within 24 hours, it still runs.

```bash
lfd schedule "0 9 * * MON-FRI" weekday-review    # 9am weekdays
lfd schedule "0 0 * * 0" weekly-cleanup          # midnight Sundays
```

## Managing loops

```bash
lfd status              # show all loops
lfd prs product-engineer   # show PRs from a loop
lfd stop product-engineer  # stop a loop
lfd rm product-engineer    # remove loop and history
```

Status output:

```
GOAL              TYPE       STATUS   ITER  TRIGGER
product-engineer  loop       running  12    continuous
api-docs          subscribe  idle     3     src/api/**
morning-triage    schedule   idle     5     0 9 * * *
```

## Goals

Goals live in `.lf/goals/`:

```bash
lfd list-goals    # see available goals
```

Each goal defines what the agent should accomplish, its quality bar, and iteration strategy.

## Branching model

Each loop gets its own branch:

```
main
  └── loop-product-engineer
        ├── product-engineer/001
        ├── product-engineer/002
        └── product-engineer/003
```

Iteration branches merge to `loop-<goal>` automatically. You review and land `loop-<goal>` → `main` when ready.

## Options

### Area override

Focus on specific paths:

```bash
lfd loop product-engineer -a src/api/
lfd subscribe "src/api/**" api-docs -r docs/api/
```

### Merge mode

```bash
lfd loop product-engineer --merge-mode pr    # accumulate PRs (default)
lfd loop product-engineer --merge-mode land  # auto-land each iteration
```

## Reference

[`lfd` commands](lfd.md) · [Configuration](config.md) · [Troubleshooting](troubleshooting.md)
