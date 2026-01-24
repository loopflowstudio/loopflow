---
layout: default
title: lfd Command Reference
---

# lfd Command Reference

`lfd` is the loopflow daemon. It runs agent loops in the background, managing iterations and PR limits.

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

Runs iterations until the PR limit is reached, then waits. Each iteration runs the flow pipeline and creates a PR.

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
| `-g, --goal` | Goal to apply (repeatable) |
| `-l, --limit` | PR limit override (default: 5) |
| `--merge-mode` | `pr` (accumulate) or `land` (auto-merge to main) |
| `-f, --foreground` | Run in foreground instead of background |

### lfd run

Run a single iteration (one-shot).

```bash
lfd run <flow> <area>
lfd run ship src/ui/
lfd run ship . -c
```

Like `lfd loop` but stops after one iteration. Good for specific tasks.

| Flag | Description |
|------|-------------|
| `-g, --goal` | Goal to apply (repeatable) |
| `-c, --clipboard` | Include clipboard content |

### lfd start

Start multiple loops at once.

```bash
lfd start                    # start all idle loops
lfd start src/api/ src/ui/  # start specific areas
lfd start --all              # include waiting loops
```

| Flag | Description |
|------|-------------|
| `-a, --all` | Include waiting loops (not just idle) |

## Stimulus Commands

### lfd subscribe

Watch for file changes on main.

```bash
lfd subscribe <flow> <area>
lfd subscribe ship src/api/
lfd subscribe ship docs/
```

When files in the area change on main, activates one iteration. The area serves as both the context for the agent and the paths to watch.

| Flag | Description |
|------|-------------|
| `-g, --goal` | Goal to apply (repeatable) |

### lfd schedule

Run on a cron schedule.

```bash
lfd schedule <flow> <area> "<cron>"
lfd schedule ship . "0 9 * * *"
lfd schedule ship src/ui/ "0 10 * * MON"
```

Schedules have a 24-hour grace period for laptops—if your computer wakes after the scheduled time but within 24 hours, it still runs.

| Flag | Description |
|------|-------------|
| `-g, --goal` | Goal to apply (repeatable) |

## Monitoring

### lfd status

Show status of all loops.

```bash
lfd status                   # all loops
lfd status <loop-id>         # specific loop details
```

Shows loop type, area, status, iteration count, and outstanding PRs.

### lfd prs

Show PRs created by an agent.

```bash
lfd prs <agent-id>
lfd prs <agent-id> -n 20
```

| Flag | Description |
|------|-------------|
| `-n, --limit` | Number of PRs to show (default: 10) |

### lfd logs

Show logs for an agent's current run.

```bash
lfd logs <agent-id>
lfd logs <agent-id> -f         # follow output
lfd logs <agent-id> -n 100     # show 100 lines
```

| Flag | Description |
|------|-------------|
| `-f, --follow` | Follow output (like tail -f) |
| `-n, --lines` | Number of lines to show (default: 50) |

### lfd list-goals

Show available goals in current repo.

```bash
lfd list-goals
```

## Managing Agents

### lfd stop

Stop a running agent.

```bash
lfd stop <agent-id>
lfd stop <agent-id> --force
```

| Flag | Description |
|------|-------------|
| `-f, --force` | Force kill with SIGKILL |

### lfd rm

Remove an agent and its history.

```bash
lfd rm <agent-id>
lfd rm <agent-id> --force
```

| Flag | Description |
|------|-------------|
| `-f, --force` | Skip confirmation prompt |

## See Also

[Background Agents](agents.md) · [Configuration](config.md)
