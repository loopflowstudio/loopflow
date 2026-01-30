---
layout: default
title: lfd Command Reference
---

# lfd Command Reference

`lfd` is the loopflow daemon. It manages waves—persistent configurations that run flows on your codebase.

## Quick Start

```bash
# One-shot: create + configure + run
lfd loop swift-falcon --area src/

# Or incrementally
lfd create swift-falcon
lfd area swift-falcon src/
lfd loop swift-falcon
```

## Creating Waves

### lfd create

Create a new wave.

```bash
lfd create swift-falcon      # create with name
lfd create                   # create with generated name
```

Creates a wave with no configuration. Use `area`, `direction`, `flow` to configure before running.

## Configuring Waves

### lfd area

Set the working area (required before running).

```bash
lfd area swift-falcon src/
lfd area swift-falcon src/api/ src/ui/    # multiple paths
```

### lfd direction

Set the direction (optional).

```bash
lfd direction swift-falcon "fix all lint errors"
lfd direction swift-falcon product-engineer    # use preset from .lf/directions/
```

### lfd flow

Set the flow (default: ship).

```bash
lfd flow swift-falcon debug
lfd flow swift-falcon ship
```

Flows are defined in `.lf/flows/<name>.yaml`.

## Running Waves

All run commands validate configuration first. Area must be set.

### lfd run

Run a single iteration.

```bash
lfd run swift-falcon
lfd run swift-falcon --area src/    # one-shot: create + configure + run
```

| Flag | Description |
|------|-------------|
| `--area` | Set area (creates wave if needed) |
| `--direction` | Set direction |
| `--flow` | Set flow |

### lfd loop

Run continuously, iteration after iteration.

```bash
lfd loop swift-falcon
lfd loop swift-falcon --area src/ --direction "improve coverage"
```

Runs until PR limit is reached, then waits. Each iteration runs the flow and creates a PR.

| Flag | Description |
|------|-------------|
| `--area` | Set area (creates wave if needed) |
| `--direction` | Set direction |
| `--flow` | Set flow |
| `-l, --limit` | PR limit (default: 5) |
| `--merge-mode` | `pr` (accumulate) or `land` (auto-merge) |

### lfd watch

Run when origin/main changes in the watched path.

```bash
lfd watch swift-falcon                    # watch area (default)
lfd watch swift-falcon --path tests/      # watch specific path
lfd watch swift-falcon --area src/ --path tests/
```

Polls origin/main for new commits. If files in the watched path changed since last run, triggers an iteration.

| Flag | Description |
|------|-------------|
| `--area` | Set working area |
| `--path` | Path to watch (default: area) |
| `--direction` | Set direction |
| `--flow` | Set flow |

### lfd cron

Run on a schedule.

```bash
lfd cron swift-falcon "0 9 * * *"         # daily at 9am
lfd cron swift-falcon "0 9 * * MON-FRI"   # weekdays at 9am
```

Has a 24-hour grace period—if your computer wakes after the scheduled time but within 24 hours, it still runs.

| Flag | Description |
|------|-------------|
| `--area` | Set working area |
| `--direction` | Set direction |
| `--flow` | Set flow |

## Stimulus Modes

| Mode | Trigger | Use Case |
|------|---------|----------|
| `run` | Manual, once | One-off task |
| `loop` | After each iteration | Continuous improvement |
| `watch` | origin/main changes | React to upstream |
| `cron` | Schedule | Daily maintenance |

## Managing Waves

### lfd list

List all waves.

```bash
lfd list
lfd list --all    # include completed/stopped
```

### lfd show

Show wave details.

```bash
lfd show swift-falcon
```

### lfd stop

Stop a running wave.

```bash
lfd stop swift-falcon
lfd stop swift-falcon --force    # SIGKILL
```

### lfd rm

Delete a wave.

```bash
lfd rm swift-falcon
lfd rm swift-falcon --force    # skip confirmation
```

## Monitoring

### lfd logs

Show logs for a wave.

```bash
lfd logs swift-falcon
lfd logs swift-falcon -f         # follow
lfd logs swift-falcon -n 100     # last 100 lines
```

### lfd prs

Show PRs created by a wave.

```bash
lfd prs swift-falcon
lfd prs swift-falcon -n 20
```

## Branch Stacking

Commands for managing dependent PRs (stacking).

### lfd next

Stack a new branch on top of current work.

```bash
lfd next                    # from within wave worktree
lfd next swift-falcon       # explicit wave name
lfd next --create-pr        # create PR first, then stack
```

Enables auto-merge on current PR, creates a new branch from HEAD, and records the base for squash-aware rebase later.

| Flag | Description |
|------|-------------|
| `-c, --create-pr` | Create PR if none exists |

### lfd rebase

Rebase stacked branch after base PR lands.

```bash
lfd rebase                  # from within wave worktree
lfd rebase swift-falcon     # explicit wave name
```

Detects if base branch was squash-merged and rebases appropriately. Uses recorded base commit to replay only stacked commits, avoiding conflicts from squash-merge.

## Daemon Management

### lfd serve

Run daemon in foreground (for debugging).

```bash
lfd serve
```

### lfd install

Install launchd service for auto-start.

```bash
lfd install
```

### lfd uninstall

Remove launchd service.

```bash
lfd uninstall
```

## See Also

[Waves](waves.md) · [Configuration](config.md)
