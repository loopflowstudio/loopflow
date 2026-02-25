---
layout: default
title: Waves
---

# Waves

A wave is **area × direction × flow**. Stimuli (triggers) are separate entities that activate the wave.

```bash
python - <<'PY'
import loopflow.api as loopflow

loopflow.create_wave("shipper", repo=".", flow="build", direction=["clarity"], area=["src/api/"])
loopflow.run_wave("shipper")
PY
```

This runs the `ship` flow on `src/api/` with the `clarity` direction—continuously, creating PRs until you stop it.

Waves are independent by default. Use a `listen` stimulus when one wave should react to another.

## Stimulus Types

| Stimulus | Runs when | Command |
|----------|-----------|---------|
| **Once** | Single run | `loopflow.run_wave(...)` |
| **Loop** | Continuously until stopped | `loopflow.run_wave(...)` with loop stimulus |
| **Watch** | Area changes on main | `loopflow.update_wave(..., stimulus=watch)` |
| **Cron** | On schedule | `loopflow.update_wave(..., stimulus=cron)` |
| **Listen** | Another wave runs | `loopflow.add_stimulus(..., kind="listen", source_wave_id=...)` |

### Once

Single execution. Run a flow once then stop.

```bash
python - <<'PY'
import loopflow.api as loopflow

loopflow.create_wave("runner", repo=".", flow="build", area=["swift/"])
loopflow.run_wave("runner")
PY
```

### Loop

Continuous work. Each iteration picks a task, runs the flow, creates a PR.

```bash
python - <<'PY'
import loopflow.api as loopflow

loopflow.create_wave("looper", repo=".", flow="build", area=["src/"])
loopflow.update_wave("looper", stimulus=loopflow.Stimulus(kind="loop"))
loopflow.run_wave("looper")
PY
```

When the PR limit is reached, the loop pauses until PRs are merged.

### Watch

React to changes. When files in the area change on main, activates one iteration.

```bash
python - <<'PY'
import loopflow.api as loopflow

loopflow.create_wave("watcher", repo=".", flow="build", area=["src/api/"])
loopflow.update_wave("watcher", stimulus=loopflow.Stimulus(kind="watch"))
loopflow.run_wave("watcher")
PY
```

The area serves as both the context for the wave and the paths to watch. When a watch triggers, the diff of changed files is included in context.

### Cron

Run on schedule. 24-hour grace period for laptops.

```bash
python - <<'PY'
import loopflow.api as loopflow

loopflow.create_wave("cronner", repo=".", flow="build", area=["."])
loopflow.update_wave("cronner", stimulus=loopflow.Stimulus(kind="cron", cron="0 9 * * *"))
loopflow.run_wave("cronner")
PY
```

### Listen

Trigger a wave when another wave runs.

```bash
python - <<'PY'
import loopflow.api as loopflow

loopflow.create_wave("ux", repo=".", flow="build", area=["docs/"])
loopflow.create_wave("infra", repo=".", flow="grind", area=["rust/"])
loopflow.add_stimulus("ux", kind="listen", source_wave_id="infra")
loopflow.run_wave("ux")
PY
```

### Multiple Stimuli

A wave can have multiple stimuli. Any stimulus firing activates the wave.

```bash
# Start with a watch stimulus
python - <<'PY'
import loopflow.api as loopflow

loopflow.create_wave("swift-falcon", repo=".", flow="build", area=["src/"])
loopflow.update_wave("swift-falcon", stimulus=loopflow.Stimulus(kind="watch"))
loopflow.run_wave("swift-falcon")
PY

# Add a cron trigger too (9am daily)
python - <<'PY'
import loopflow.api as loopflow

loopflow.update_wave("swift-falcon", stimulus=loopflow.Stimulus(kind="cron", cron="0 9 * * *"))
PY

# List all triggers
lfq show swift-falcon

# Disable one trigger
python - <<'PY'
import loopflow.api as loopflow

loopflow.update_wave("swift-falcon", status="paused")
PY
```

When a wave is already running and another stimulus fires, the activation queues. Watch triggers coalesce—multiple commits combine into a single activation with a combined diff.

---

## Quick Start

```bash
lfd install                      # one-time: install daemon
lfq list                         # check progress
```

Or run manually: `lfd serve`

## One-Shot

Run exactly one iteration using the once stimulus:

```bash
python - <<'PY'
import loopflow.api as loopflow

loopflow.create_wave("runner", repo=".", flow="build", area=["src/"])
loopflow.run_wave("runner")
PY
```

## Managing Waves

```bash
lfq list                # show all waves
lfq logs <name>         # show logs
lfq stop <name>         # stop a wave
lfq delete <name>       # remove wave and history
```

Status output:

```
ID       STIMULUS   AREA                             STATUS     ITER  REPO
abc1234  loop       src/ [ship] [clarity]           running    12    ~/repo
```

## Next

[Get Started →](getting-started.md)

## Reference

[`lfd` commands](lfd.md) · [Configuration](config.md) · [Troubleshooting](troubleshooting.md)
