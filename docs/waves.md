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

Projects and tasks live in Linear and sync into the local SQLite registry. Run
`lf pm sync`, then run the agent: it reads its projects,
tasks, and memory; pick the next move; execute it inline or place a justified
child loop; watch the PR; and fold what changed back into memory.

```bash
lf serve shipper            # start the wave agent
tmux ls                    # the wave agent and every worker it launches
tmux attach -r -t <name>   # inspect one without direct control
```

The wave agent resolves the sole blocker inline. It creates a child loop only
for a strict subset that needs its own repeated lifecycle or useful parallelism:

```bash
lf --wave shipper loop build "add retry to token refresh"
lf --wave shipper loop build "audit retry callers" --detach
```

A worker runs the flow in its own worktree, opens a PR, and reports back. It inherits the wave's `GOAL.md` and `MEMORY.md`, so it builds with the wave's intent in view.

Each loop gets a fresh sibling worktree — `<repo>.<wave>.<run-id>` — off the
default branch, with its own branch and PR:

```bash
lf --wave shipper loop task "…"           # foreground: caller-owned
lf --wave shipper loop task "…" --detach  # background: server-owned
```

Both forms create the same placed worktree. `--detach` changes attention and
ownership, not execution: the server launches a headless loop and the caller
returns immediately. It requires an already-running server and is useful only
when the parent has another move; otherwise keep the loop foreground.

Waves are independent by default. Humans steer a served mind with `lf chat`;
agents report on its bus with `lf radio pub --channel <name> "…"`, even while the
wave sleeps.

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
