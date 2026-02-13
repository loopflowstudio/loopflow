---
layout: default
title: lfd Daemon Reference
---

# lfd Daemon Reference

`lfd` runs the wave daemon. It serves the HTTP API on `127.0.0.1:2486` by default.

## Run the daemon

```bash
lfd serve
```

## Install (launchd)

```bash
lfd install
```

## Uninstall

```bash
lfd uninstall
```

## Query + manage waves

Use `lfq` for CLI workflows and `loopflow` for Python orchestration:

```bash
lfq list
lfq logs engbot
```

```python
import loopflow.api as loopflow

loopflow.create_wave("engbot", repo=".")
loopflow.run_wave("engbot")
```

## Configuration

Environment variables:

```
LFD_URL   # full base URL (overrides host/port)
LFD_HOST  # default 127.0.0.1
LFD_PORT  # default 2486
```

If `LFD_URL` is set, it takes precedence.

`lfd` also reads `~/.lf/lfd.yaml` for daemon settings:

```yaml
executor:
  type: local # or docker
  image: loopflow/agent:latest
  credentials:
    env: ["ANTHROPIC_API_KEY", "CODEX_API_KEY"]
    mounts:
      - ~/.claude:/home/agent/.claude
      - ~/.codex/auth.json:/home/agent/.codex/auth.json
```

`credentials.mounts` uses `host_path:container_path`. `~/...` is expanded to your home directory, and `container_path` must be absolute.

Environment overrides:

```bash
LFD_EXECUTOR_TYPE=docker
LFD_EXECUTOR_IMAGE=loopflow/agent:latest
```
