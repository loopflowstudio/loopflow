---
layout: default
title: Background Agents
---

# Background Agents

An agent is **area × goal × flow × stimulus**.

```bash
lfd loop ship src/api/ --goal product-engineer
```

This runs the `ship` flow on `src/api/` with the `product-engineer` goal—continuously, creating PRs until you stop it.

## Stimulus Types

| Stimulus | Runs when | Command |
|----------|-----------|---------|
| **Once** | Single run | `lfd run` |
| **Loop** | Continuously until stopped | `lfd loop` |
| **Watch** | Area changes on main | `lfd subscribe` |
| **Cron** | On schedule | `lfd schedule` |

### Once

Single execution. Run a flow once then stop.

```bash
lfd run ship swift/                       # one-off iteration
lfd run ship swift/ -g product-engineer   # with a goal
lfd run ship . -c                         # whole repo with clipboard
```

### Loop

Continuous work. Each iteration picks a task, runs the flow, creates a PR.

```bash
lfd loop ship src/                        # run continuously
lfd loop ship src/ --limit 3              # max 3 outstanding PRs
lfd loop ship src/ --merge-mode land      # auto-land to main
```

When the PR limit is reached, the loop pauses until PRs are merged.

### Watch

React to changes. When files in the area change on main, activates one iteration.

```bash
lfd subscribe ship src/api/               # watch src/api/ for changes
lfd subscribe ship docs/                  # watch docs/ for changes
```

The area serves as both the context for the agent and the paths to watch.

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

Run exactly one iteration using the once stimulus:

```bash
lfd run ship src/                       # single iteration
lfd run ship src/ -g product-engineer   # with a goal
lfd run ship src/ -c                    # with clipboard
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
ID       STIMULUS   AREA                             STATUS     ITER  REPO
abc1234  loop       src/ [ship] [product-engineer]  running    12    ~/repo
```

## Next

[Get Started →](getting-started.md)

## Reference

[`lfd` commands](lfd.md) · [Configuration](config.md) · [Troubleshooting](troubleshooting.md)
