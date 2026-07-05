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
lfq wave run shipper       # start (or attach to) the wave agent
lfq sessions               # the wave agent and every worker it launches
lfq attach <session-id>    # jump into one over tmux
```

The wave agent coordinates; workers do the implementation. When the agent picks a substantial task it dispatches one:

```bash
lfq worker run shipper --flow build --task "add retry to token refresh"
```

A worker runs the flow in its own worktree, opens a PR, and reports back. It inherits the wave's `GOAL.md` and `MEMORY.md`, so it builds with the wave's intent in view.

By default each worker gets a fresh sibling worktree — `<repo>.<wave>.<run-id>` — off the default branch, with its own branch and PR. Placement flags change that:

```bash
lfq worker run shipper --flow build --task "…" --pool           # run in the wave's shared worktree
lfq worker run shipper --flow build --task "…" --stack <run-id> # fork from that run's branch
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

Crons schedule supplementary flows on a wave. They do not replace the wave's primary flow, and they do not consume the wave's `workers` budget.

```markdown
<!-- wave/shipper/GOAL.md -->
---
primary_flow: build
workers: 2
mode: loop
metrics:
  - backlog is empty
---

Run one loop iteration for the shipper wave.
```

Configure cron schedules through the live API:

```python
import loopflow.api as loopflow

loopflow.create_wave(
    "shipper",
    repo=".",
    flow="build",
    crons=[{"flow": "sync", "schedule": "0 0 1 * *"}],
)
```

Use `workers: 0` in `GOAL.md` for waves that only run from cron schedules:

```markdown
<!-- wave/governance/GOAL.md -->
---
primary_flow: garden
workers: 0
mode: manual
---

Run one loop iteration for the governance wave.
```

Then configure the cron schedules in lfd:

```python
import loopflow.api as loopflow

loopflow.create_wave(
    "governance",
    repo=".",
    flow="garden",
    crons=[
        {"flow": "govern-identity", "schedule": "0 0 * * 0"},
        {"flow": "govern-coordination", "schedule": "0 0 * * *"},
    ],
)
```

## Triggers

A trigger pairs a signal (what changed) with a flow (what to run). They fire regardless of mode.

| Signal | What changed | Default flow |
|--------|--------------|--------------|
| **repo** | Paths changed on main | `integrate` |
| **wave** | Another wave completed | `build` |
| **ci_failure** | CI failed on a wave PR | `ci-fix` |

Every new wave ships with two default triggers: `repo` (whole repo → integrate) and `ci_failure` → `ci-fix`.

### Repo

React to changes on main. By default watches the whole repo and runs `integrate` (rebase + update wave content). Add a trigger with specific paths to watch a subset.

```bash
python - <<'PY'
import loopflow.api as loopflow

# Watch specific paths with a custom flow
loopflow.create_wave("syncer", repo=".", flow="build")
loopflow.add_trigger("syncer", signal="repo", flow="build")
loopflow.run_wave("syncer")
PY
```

When a repo trigger fires, the diff of changed files is included in context.

### Wave

React to another wave completing. More deliberate than a repo trigger — signals that specific changes are likely relevant.

```bash
python - <<'PY'
import loopflow.api as loopflow

loopflow.create_wave("ux", repo=".", flow="build")
loopflow.create_wave("infra", repo=".", flow="govern-control")
loopflow.add_trigger("ux", signal="wave", source_wave_id="infra")
loopflow.run_wave("ux")
PY
```

### CiFailure

Runs `ci-fix` when CI fails on a wave's PR. Ships as a default trigger — no need to declare it.

### Multiple Triggers

Triggers are a list. Multiple triggers of the same signal are fine — watch different paths, react to different waves.

```bash
python - <<'PY'
import loopflow.api as loopflow

loopflow.create_wave("swift-falcon", repo=".", flow="build")
loopflow.add_trigger("swift-falcon", signal="wave", source_wave_id="infra")
loopflow.run_wave("swift-falcon")
PY

# List all triggers
lfq show swift-falcon

# Stop the wave
lfq stop swift-falcon
```

When a wave is already running and another trigger fires, the activation queues. Repo triggers coalesce—multiple commits combine into a single activation with a combined diff.

---

## Quick Start

```bash
lfd install                      # one-time: install daemon
lfq list                         # check progress
```

Or run manually: `lfd serve`

## Managing Waves

```bash
lfq list                # show all waves
lfq sessions            # show live terminal sessions
lfq attach <session-id> # attach to one over tmux
lfq logs <name>         # show logs
lfq stop <name>         # stop a wave
lfq delete <name>       # remove wave and history
```

Status output:

```
ID       NAME     MODE   STATUS   ITER  REPO
abc1234  shipper  loop   running  12    ~/repo
```

## Next

[Wave Authoring →](wave-authoring.md) · [Get Started →](getting-started.md)

## Reference

[`lfd` commands](lfd.md) · [Configuration](config.md) · [Troubleshooting](troubleshooting.md)
