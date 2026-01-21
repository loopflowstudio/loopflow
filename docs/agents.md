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
lfd loop ship Maestro/           # start a loop
lfd status                       # check progress
lfd prs <loop-id>                # see created PRs
```

## Concepts

**Loop**: A long-running agent working toward a goal. Each iteration picks a task, runs a flow/pipeline (design → implement → polish), creates a PR.

**Goal**: A markdown file in `.lf/goals/` describing what the agent should accomplish.

**Flow**: A pipeline name defined in `.lf/flows/<name>.py`.

**Iteration**: One cycle of work. Pick task, execute, create PR, repeat.

## Starting the daemon

```bash
lfd install    # installs launchd service, auto-starts on login
lfd status     # verify it's running
```

Or run manually: `lfd serve`

## Continuous loops

```bash
lfd loop ship Maestro/                   # run continuously
lfd loop ship Maestro/ --limit 3         # max 3 outstanding PRs
lfd loop ship Maestro/ --merge-mode land # auto-land to main
```

When the PR limit is reached, the loop pauses until PRs are merged.

## One-shot flows

Run exactly one iteration:

```bash
lfd flow ship Maestro/             # single iteration
lfd flow ship Maestro/ -p spec.md  # with project file
lfd flow ship Maestro/ -v          # with clipboard
```

## Triggers

Beyond continuous loops, agents can react to events.

### Subscribe to file changes

```bash
lfd subscribe ship src/api/ -p src/api
```

When files change under the watched path on main, runs one iteration.

```bash
lfd subscribe ship src/api/ -p src/api/routes
lfd subscribe ship . -p schema.graphql -p src/resolvers
```

### Schedule with cron

```bash
lfd schedule ship . "0 9 * * *"
```

Schedules have a 24-hour grace period—if your laptop wakes after the scheduled time but within 24 hours, it still runs.

```bash
lfd schedule ship . "0 9 * * MON-FRI"     # 9am weekdays
lfd schedule ship . "0 0 * * 0"           # midnight Sundays
```

## Managing loops

```bash
lfd status              # show all loops
lfd prs <loop-id>       # show PRs from a loop
lfd stop <loop-id>      # stop a loop
lfd rm <loop-id>        # remove loop and history
```

Status output:

```
ID       TYPE       AREA                        STATUS     ITER  REPO
abc1234  loop       Maestro/ [ship] [adaptive]  running    12    ~/repo
```

## Goals

Goals live in `.lf/goals/`:

```bash
lfd list-goals    # see available goals
```

Each goal defines what the agent should accomplish, its quality bar, and iteration strategy.

## Merge mode

```bash
lfd loop ship Maestro/ --merge-mode pr    # accumulate PRs (default)
lfd loop ship Maestro/ --merge-mode land  # auto-land each iteration
```

## Reference

[`lfd` commands](lfd.md) · [Configuration](config.md) · [Troubleshooting](troubleshooting.md)
