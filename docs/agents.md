---
layout: default
title: Background Agents
---

# Background Agents

An agent is **flow × area × voice**.

```bash
lfd loop ship src/api/ --voice architect
```

This runs the `ship` flow on `src/api/` through the `architect` voice—continuously, creating PRs until you stop it.

## The Three Modes

| Mode | Runs when | Command |
|------|-----------|---------|
| **Loop** | Continuously until stopped | `lfd loop` |
| **Watch** | Paths change on main | `lfd subscribe` |
| **Cron** | On schedule | `lfd schedule` |

### Loop

Continuous work. Each iteration picks a task, runs the flow, creates a PR.

```bash
lfd loop ship src/                        # run continuously
lfd loop ship src/ --limit 3              # max 3 outstanding PRs
lfd loop ship src/ --merge-mode land      # auto-land to main
```

When the PR limit is reached, the loop pauses until PRs are merged.

### Watch

React to changes. When files change on main, run one iteration.

```bash
lfd subscribe ship src/api/ -p src/api
lfd subscribe ship . -p schema.graphql -p src/resolvers
```

### Cron

Run on schedule. 24-hour grace period for laptops.

```bash
lfd schedule ship . "0 9 * * *"           # 9am daily
lfd schedule ship . "0 9 * * MON-FRI"     # 9am weekdays
```

---

## Quick Start

```bash
lfd install                      # one-time: install daemon
lfd loop ship src/               # start a loop
lfd status                       # check progress
lfd prs <loop-id>                # see created PRs
```

Or run manually: `lfd serve`

## One-Shot

Run exactly one iteration:

```bash
lfd flow ship src/               # single iteration
lfd flow ship src/ -p spec.md    # with project file
lfd flow ship src/ -c            # with clipboard
```

## Managing Agents

```bash
lfd status              # show all agents
lfd prs <id>            # show PRs from an agent
lfd stop <id>           # stop an agent
lfd rm <id>             # remove agent and history
```

Status output:

```
ID       TYPE       AREA                    STATUS     ITER  REPO
abc1234  loop       src/ [ship] [architect] running    12    ~/repo
```

## Reference

[`lfd` commands](lfd.md) · [Configuration](config.md) · [Troubleshooting](troubleshooting.md)
