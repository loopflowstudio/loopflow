---
layout: default
title: Waves
---

# Waves

A wave is **area × direction × flow**. Mode controls the primary execution pattern. Crons schedule supplementary flows. Triggers fire flows in response to signals.

```bash
python - <<'PY'
import loopflow.api as loopflow

loopflow.create_wave("shipper", repo=".", flow="build", direction=["clarity"], area=["src/api/"])
loopflow.run_wave("shipper")
PY
```

This creates a wave with the `build` flow on `src/api/` and the `clarity` direction.

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

```yaml
# wave/shipper/shipper.yaml
flow: build
workers: 2
mode: loop
crons:
  - flow: wave-polish
    schedule: "0 0 * * 1"
  - flow: wave-reduce
    schedule: "0 0 1 * *"
```

Use `workers: 0` for waves that only run from cron schedules:

```yaml
# wave/governance/governance.yaml
flow: garden
workers: 0
mode: manual
crons:
  - flow: govern-identity
    schedule: "0 0 * * 0"
  - flow: govern-coordination
    schedule: "0 0 * * *"
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
loopflow.create_wave("syncer", repo=".", flow="build", area=["docs/"])
loopflow.add_trigger("syncer", signal="repo", paths=["src/api/"], flow="build")
loopflow.run_wave("syncer")
PY
```

When a repo trigger fires, the diff of changed files is included in context.

### Wave

React to another wave completing. More deliberate than a repo trigger — signals that specific changes are likely relevant.

```bash
python - <<'PY'
import loopflow.api as loopflow

loopflow.create_wave("ux", repo=".", flow="build", area=["docs/"])
loopflow.create_wave("infra", repo=".", flow="grind", area=["rust/"])
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

loopflow.create_wave("swift-falcon", repo=".", flow="build", area=["src/"])
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
lfq logs <name>         # show logs
lfq stop <name>         # stop a wave
lfq delete <name>       # remove wave and history
```

Status output:

```
ID       MODE   AREA                             STATUS     ITER  REPO
abc1234  loop   src/ [ship] [clarity]             running    12    ~/repo
```

## Next

[Wave Authoring →](wave-authoring.md) · [Get Started →](getting-started.md)

## Reference

[`lfd` commands](lfd.md) · [Configuration](config.md) · [Troubleshooting](troubleshooting.md)
