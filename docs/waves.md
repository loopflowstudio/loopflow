---
layout: default
title: Waves
---

# Waves

A wave is a named agent with a goal. You author two files under `wave/<name>/`:

| File | Holds |
|------|-------|
| **`GOAL.md`** | The wave's intent and loop prompt — what it's for, how it judges progress |
| **`MEMORY.md`** | What the wave remembers between loops — written by the wave agent |

Run the agent and it works a loop: read the roadmap and memory, pick the next move, dispatch a worker to build it, watch the PR, and fold what changed back into memory.

```bash
lf wave shipper            # start the wave agent
tmux ls                    # the wave agent and every worker it launches
tmux attach -t <name>      # jump into one
```

The wave agent coordinates; workers do the implementation. When the agent picks a substantial task it dispatches one:

```bash
lf build "add retry to token refresh" --wave shipper --dispatch
```

A worker runs the flow in its own worktree, opens a PR, and reports back. It inherits the wave's `GOAL.md` and `MEMORY.md`, so it builds with the wave's intent in view.

By default each worker gets a fresh sibling worktree — `<repo>.<wave>.<run-id>` — off the default branch, with its own branch and PR. Placement flags change that:

```bash
lf build "…" --wave shipper --dispatch        # separate worktree for this task
lf build "…" --wave shipper --stack <run-id> # stack on that run's branch
lf build "…" --wave shipper --fork           # independent branch from the review base
```

`--dispatch` creates a placed worktree and blocks until the normal `lf` run exits. `--stack` starts dependent work on top of an unlanded run's branch; `--fork` starts an independent branch from the review base.

Waves are independent by default. When one process needs to report into a wave, post into its thread — `lf chat --wave <name> "…"` works from any process, including another wave's flowloop.

## Crons

Crons schedule supplementary flows on a wave. They live in `GOAL.md` frontmatter and are read by the wave's resident flowloop: when a schedule comes due while the flowloop is idle, it opens a system pass ("cron due: <flow> — dispatch it") and dispatches the flow with judgment. Edits to the file land without a restart.

```markdown
<!-- wave/shipper/GOAL.md -->
---
workers: 2
crons:
  - flow: sync
    schedule: "0 0 0 1 * * *"
---

## Objective

Run one loop iteration for the shipper wave.

## Measures

- **Key Results**: backlog is empty.

## Process

Read the roadmap and dispatch the appropriate flow for the next useful move.
```

Schedules use 6/7-field cron syntax (seconds first). A schedule that comes due mid-turn fires at the next turn boundary; occurrences older than 24 hours are missed, not replayed.

Use `workers: 0` in `GOAL.md` for waves that only run from cron schedules:

```markdown
<!-- wave/governance/GOAL.md -->
---
workers: 0
crons:
  - flow: govern-identity
    schedule: "0 0 0 * * Sun *"
  - flow: govern-coordination
    schedule: "0 0 0 * * * *"
---

Run one loop iteration for the governance wave.
```

---

## Quick Start

```bash
lfd install                      # one-time: install daemon
```

Or run manually: `lfd serve`. Watch progress in Loopflow.

## Managing Waves

```bash
lf wave <name>          # start the wave agent (Ctrl-C to stop)
tmux ls                 # live sessions — the wave agent and its workers
tmux attach -t <name>   # attach to one; agent output lives here
```

To remove a wave, delete `wave/<name>/` and its worktree (`lf op wt remove <name>`).

## Next

[Wave Authoring →](wave-authoring.md) · [Get Started →](getting-started.md)

## Reference

[`lfd` commands](lfd.md) · [Configuration](config.md) · [Troubleshooting](troubleshooting.md)
