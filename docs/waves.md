---
layout: default
title: Waves
---

# Waves

A wave is a durable operating context. Projects are measured bets inside a wave.
Tasks are the concrete work that advances a project.

You author wave files under `wave/<name>/`:

| File | Holds |
|------|-------|
| **`GOAL.md`** | The wave's intent and loop prompt — what it's for, how it judges progress |
| **`MEMORY.md`** | What the wave remembers between loops — written by the wave agent |
| **`projects/*.md`** | One measured bet per file, with KRs and closure criteria |

Tasks live in Linear. Run the agent and it works a loop: read its projects,
tasks, and memory; pick the next move; inhabit or delegate a loop; watch the PR; and fold
what changed back into memory.

```bash
lf serve shipper            # start the wave agent
tmux ls                    # the wave agent and every worker it launches
tmux attach -r -t <name>   # inspect one without direct control
```

The wave agent can inhabit one loop and delegate self-sufficient work:

```bash
lf loop build "add retry to token refresh" --wave shipper
lf loop build "audit retry callers" --wave shipper --detach
```

A worker runs the flow in its own worktree, opens a PR, and reports back. It inherits the wave's `GOAL.md` and `MEMORY.md`, so it builds with the wave's intent in view.

Each loop gets a fresh sibling worktree — `<repo>.<wave>.<run-id>` — off the
default branch, with its own branch and PR:

```bash
lf loop task "…" --wave shipper           # foreground: block until done
lf loop task "…" --wave shipper --detach  # background: server-owned
```

Both forms create the same placed worktree. `--detach` changes attention and
ownership, not execution: the server launches a headless loop and the caller
returns immediately.

Waves are independent by default. When one process needs to report into a wave, post into its thread — `lf chat --wave <name> "…"` works from any process, including another wave's loop.

## Crons

Crons schedule supplementary flows on a wave. They live in `GOAL.md` frontmatter and are read by the wave's resident loop: when a schedule comes due while the loop is idle, it opens a system pass ("cron due: <flow> — dispatch it") and dispatches the flow with judgment. Edits to the file land without a restart.

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

Read the live tasks and dispatch the appropriate flow for the next useful move.
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
lf serve <name>          # start the wave agent (Ctrl-C to stop)
tmux ls                 # live sessions — the wave agent and its workers
tmux attach -r -t <name>   # inspect one; stdin stays closed
```

To remove a wave, delete `wave/<name>/` and its worktree (`lf wt remove <name>`).

## Next

[Wave Authoring →](wave-authoring.md) · [Get Started →](getting-started.md)

## Reference

[`lfd` commands](lfd.md) · [Configuration](config.md) · [Troubleshooting](troubleshooting.md)
