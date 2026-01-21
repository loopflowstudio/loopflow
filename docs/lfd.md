---
layout: default
title: lfd Command Reference
---

# lfd Command Reference

`lfd` is the loopflow daemon. It runs agent loops in the background, managing goals, iterations, and PR limits.

## Basic Usage

```bash
lfd                          # show status (default)
lfd loop <flow> <area>       # start continuous loop
lfd flow <flow> <area>       # run single iteration
lfd status                   # show all loops
```

## Daemon Management

### lfd serve

Run daemon in foreground.

```bash
lfd serve
```

Runs the daemon process directly. Useful for debugging or when you don't want launchd management.

### lfd install

Install launchd service for auto-start.

```bash
lfd install
```

Creates a launchd plist so the daemon starts automatically at login.

### lfd uninstall

Remove launchd service.

```bash
lfd uninstall
```

Stops the daemon and removes the launchd plist.

## Starting Loops

### lfd loop

Start a continuous homeostasis loop.

```bash
lfd loop <flow> <area>
lfd loop ship src/api/
lfd loop ship .
```

Runs iterations until the PR limit is reached, then waits. Each iteration picks work based on the goal, runs a pipeline, and creates a PR.

Flows live in `.lf/flows/<name>.py`:

```yaml
---
steps:
  - implement
  - rebase
  - polish
  - draft_commit
pr: true
---
```

| Flag | Description |
|------|-------------|
| `-g, --goal` | Goal to add (repeatable) |
| `-l, --limit` | PR limit override (default: 5) |
| `--merge-mode` | `pr` (accumulate) or `land` (auto-merge to main) |
| `-f, --foreground` | Run in foreground instead of background |

### lfd flow

Run a single iteration (one-shot).

```bash
lfd flow <flow> <area>
lfd flow ship Maestro/ -p spec.md
lfd flow ship . -v
```

Like `lfd loop` but stops after one iteration. Good for specific tasks.

| Flag | Description |
|------|-------------|
| `-g, --goal` | Goal to add (repeatable) |
| `-p, --project` | Project/prompt file to include |
| `-v, --paste` | Include clipboard content |

### lfd start

Start multiple loops at once.

```bash
lfd start                    # start all idle loops
lfd start src/api/ Maestro/  # start specific areas
lfd start --all              # include waiting loops
```

| Flag | Description |
|------|-------------|
| `-a, --all` | Include waiting loops (not just idle) |

## Triggers

### lfd subscribe

Watch for file changes on main.

```bash
lfd subscribe <flow> <area> -p <path>
lfd subscribe ship src/api/ -p src/api
lfd subscribe ship . -p schema.graphql -p src/resolvers
```

When files change under the watched paths on main, triggers one iteration.

| Flag | Description |
|------|-------------|
| `-p, --path` | Path to watch (repeatable) |
| `-g, --goal` | Goal to add (repeatable) |

### lfd schedule

Run on a cron schedule.

```bash
lfd schedule <flow> <area> "<cron>"
lfd schedule ship . "0 9 * * *"
lfd schedule ship Maestro/ "0 10 * * MON"
```

Schedules have a 24-hour grace period for laptops—if your computer wakes after the scheduled time but within 24 hours, it still runs.

| Flag | Description |
|------|-------------|
| `-g, --goal` | Goal to add (repeatable) |

## Monitoring

### lfd status

Show status of all loops.

```bash
lfd status                   # all loops
lfd status <loop-id>         # specific loop details
```

Shows loop type, area, status, iteration count, and outstanding PRs.

### lfd prs

Show PRs created by a loop.

```bash
lfd prs <loop-id>
lfd prs <loop-id> -n 20
```

| Flag | Description |
|------|-------------|
| `-n, --limit` | Number of PRs to show (default: 10) |

### lfd list-goals

Show available goals in the current repo.

```bash
lfd list-goals
```

Lists goals from `.lf/goals/` with their area and flow configuration.

## Managing Loops

### lfd stop

Stop a running loop.

```bash
lfd stop <loop-id>
lfd stop <loop-id> --force
```

| Flag | Description |
|------|-------------|
| `-f, --force` | Force kill with SIGKILL |

### lfd rm

Remove a loop and its history.

```bash
lfd rm <loop-id>
lfd rm <loop-id> --force
```

| Flag | Description |
|------|-------------|
| `-f, --force` | Skip confirmation prompt |

## Goals

Goals are markdown files in `.lf/goals/` describing what an agent should accomplish. See [Background Agents](agents.md) for details.

## See Also

[Background Agents](agents.md) · [Configuration](config.md)
