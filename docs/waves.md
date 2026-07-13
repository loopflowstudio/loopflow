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

Tasks live in Linear. Run the agent and it reads its Projects, tasks, and
memory; selects a Project before creating or selecting the next Task; starts
one durable Task Session; and folds the linked result back into memory.

```bash
lf serve shipper            # start the wave agent
tmux ls                    # live Wave, Project, and Task Sessions
lf task attach INF-123     # audited writable task control prompt
```

Every concrete file-writing change begins with a Linear task:

```bash
lf task start "add retry to token refresh" --project <linear-project-id>
lf task run INF-123
lf task follow-up INF-123 "also audit retry callers"
```

A Task Session runs in one immutable worktree, opens one PR to `main`, and
reports linked events to its Wave. It inherits the Wave's `GOAL.md` and
`MEMORY.md` plus its Project definition and KRs.

Each Task Session gets a sibling worktree off `main`, with its own branch and
PR:

```bash
lf task status INF-123
lf task interrupt INF-123 --message "take the smaller approach"
lf task receipt COMMAND_ID --until incorporated --timeout 30s --json
lf task acknowledge INF-123 --directive 2 --summary "the smaller approach is active"
lf task decide INF-123 DECISION_ID approve
lf task wait INF-123
```

The Wave stays directly steerable while several independent tasks run. Task
events enter its inbox as typed observations and wake it once. Decision answers,
review feedback, and CI repair resume the same Task Session and provider history.

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

Keep releases routine and recoverable.

## Measures

- **Key Results**: four weekly releases complete without manual repair.

## Process

Clarify the portfolio, direct the next Project or Task, and judge the evidence.
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

Keep governance checks current and actionable.
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
tmux ls                 # live Wave, Project, and Task Sessions
tmux attach -r -t <name>   # inspect one; stdin stays closed
```

To remove a wave, stop it, then delete `wave/<name>/`.

## Next

[Wave Authoring →](wave-authoring.md) · [Get Started →](getting-started.md)

## Reference

[`lfd` commands](lfd.md) · [Configuration](config.md) · [Troubleshooting](troubleshooting.md)
