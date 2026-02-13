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
LFD_HTTP_ADDR     # daemon listen address (default 127.0.0.1:2486)
LFD_STORAGE       # sqlite (default) or postgres
LFD_DB_PATH       # sqlite path override
LFD_DATABASE_URL  # required when LFD_STORAGE=postgres
LFD_MAX_SLOTS     # concurrent run slots
```

`lfd` also reads `~/.lf/lfd.yaml` for daemon settings:

```yaml
executor:
  type: local # or docker
  image: loopflow/agent:latest # base image for generated .lf/Dockerfile
  credentials:
    env: ["ANTHROPIC_API_KEY", "CODEX_API_KEY"]
    mounts:
      - claude
      - codex
      - ssh
```

`credentials.mounts` uses named allowlisted mounts only:

- `claude` → `~/.claude`
- `codex` → `~/.codex`
- `gemini` → `~/.config/gemini`
- `gitconfig` → `~/.gitconfig`
- `ssh` → `~/.ssh`
- `gnupg` → `~/.gnupg`

Names are mounted read-only into `/home/agent/...`. Raw `host:container` mount strings are rejected.

Environment overrides:

```bash
LFD_EXECUTOR_TYPE=docker
LFD_EXECUTOR_IMAGE=loopflow/agent:latest # base image for generated Dockerfiles
```

When `executor.type` is `docker`, `lfd` runs steps from a persistent Docker volume per repo (not a host bind mount). Each run uses a shared clone plus per-wave worktrees inside the volume and applies hygiene before execution (`git fetch`, `git reset --hard`, `git clean -fdx`).

Docker mode also:

- supports `fork` steps with `select: all` by creating isolated fork worktrees
- builds a repo-specific image tag (`lfd-agent-<repo-key>:latest`) from `.lf/Dockerfile`
- reattaches to running agent containers after daemon restart
