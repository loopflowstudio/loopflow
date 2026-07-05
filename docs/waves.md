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
lf q worker run shipper --flow build --task "add retry to token refresh"
```

A worker runs the flow in its own worktree, opens a PR, and reports back. It inherits the wave's `GOAL.md` and `MEMORY.md`, so it builds with the wave's intent in view.

By default each worker gets a fresh sibling worktree — `<repo>.<wave>.<run-id>` — off the default branch, with its own branch and PR. Placement flags change that:

```bash
lf q worker run shipper --flow build --task "…" --pool           # run in the wave's shared worktree
lf q worker run shipper --flow build --task "…" --stack <run-id> # fork from that run's branch
```

`--pool` runs in the wave's shared `<repo>.<wave>` worktree: pooled workers share one branch, so concurrent pooled dispatches can collide — use it only when workers must see each other's edits live. `--stack` starts dependent work on top of an unlanded run's branch; the new run targets the parent's branch instead of main.

Waves are independent by default. Add a `wave` trigger when one wave should react to another.

## Modes

The wave's `mode` controls its primary execution pattern.

| Mode | Behavior |
|------|----------|
| **manual** | Single run, then stop |
| **loop** | Continuously until stopped |

### Manual

Single execution. Run a flow once, then stop.

### Loop

Continuous work. Each iteration picks a task, runs the flow, creates a PR. When the PR limit is reached, the loop pauses until PRs are merged.

## Crons

Crons schedule supplementary flows on a wave. They live in `GOAL.md` frontmatter and are read by the wave's resident mind: when a schedule comes due while the mind is idle, it opens a system turn ("cron due: <flow> — dispatch it") and dispatches the flow with judgment. Edits to the file land without a restart.

```markdown
<!-- wave/shipper/GOAL.md -->
---
primary_flow: build
workers: 2
mode: loop
crons:
  - flow: sync
    schedule: "0 0 0 1 * * *"
metrics:
  - backlog is empty
---

Run one loop iteration for the shipper wave.
```

Schedules use 6/7-field cron syntax (seconds first). A schedule that comes due mid-turn fires at the next turn boundary; occurrences older than 24 hours are missed, not replayed.

Use `workers: 0` in `GOAL.md` for waves that only run from cron schedules:

```markdown
<!-- wave/governance/GOAL.md -->
---
primary_flow: garden
workers: 0
mode: manual
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

Or run manually: `lfd serve`. Watch progress in Concerto.

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
