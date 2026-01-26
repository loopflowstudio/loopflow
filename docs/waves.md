---
layout: default
title: Waves
---

# Waves

A wave is **area × direction × flow × stimulus**.

```bash
lfd loop ship src/api/ --direction product-engineer
```

This runs the `ship` flow on `src/api/` with the `product-engineer` direction—continuously, creating PRs until you stop it.

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
lfd run ship swift/ -d product-engineer   # with a direction
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

The area serves as both the context for the wave and the paths to watch.

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
lfd run ship src/ -d product-engineer   # with a direction
lfd run ship src/ -c                    # with clipboard
```

## Managing Waves

```bash
lfd status              # show all waves
lfd prs <id>            # show PRs from a wave
lfd stop <id>           # stop a wave
lfd rm <id>             # remove wave and history
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
