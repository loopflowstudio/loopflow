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
LFD_AUTH_PROVIDER # local (default), static, or loopflow.studio
LFD_AUTH_TOKEN    # required when LFD_AUTH_PROVIDER=static
LFD_GITHUB_WEBHOOK_SECRET  # required for /v0/hooks/github signature verification
LFD_GITHUB_TOKEN           # optional; enables startup/on-demand CI polling
```

`lfd` also reads `~/.lf/lfd.yaml` for daemon settings:

```yaml
auth:
  provider: local # local (default), static, or loopflow.studio
  token: your-static-token # required when provider=static
  base_url: https://auth.loopflow.studio # used by loopflow.studio provider
executor:
  type: local # or docker
  image: loopflow/agent:latest # base image for generated .lf/Dockerfile
  credentials:
    env: ["ANTHROPIC_API_KEY", "CODEX_API_KEY"]
    mounts:
      - claude
      - codex
      - ssh
github:
  webhook_secret: your-webhook-secret
  token: ghp_xxx # optional, used for startup /check-ci polling
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
LFD_AUTH_PROVIDER=static
LFD_AUTH_TOKEN=your-static-token
LFD_EXECUTOR_TYPE=docker
LFD_EXECUTOR_IMAGE=loopflow/agent:latest # base image for generated Dockerfiles
LFD_GITHUB_WEBHOOK_SECRET=your-webhook-secret
LFD_GITHUB_TOKEN=ghp_xxx
```

Auth behavior:

- Loopback clients (`127.0.0.1`) always bypass auth.
- `auth.provider=local` rejects non-loopback requests with `403`.
- `auth.provider=static` and `auth.provider=loopflow.studio` require `Authorization: Bearer <token>` on non-loopback requests.

When `executor.type` is `docker`, `lfd` runs steps from a persistent Docker volume per repo (not a host bind mount). Each run uses a shared clone plus per-wave worktrees inside the volume and applies hygiene before execution (`git fetch`, `git reset --hard`, `git clean -fdx`).

Docker mode also:

- builds a repo-specific image tag (`lfd-agent-<repo-key>:latest`) from `.lf/Dockerfile`
- runs `install-loopflow.sh --install` in generated Dockerfiles when available in the base image
- treats `.lf/env-setup.sh` as project-owned setup; call `install-loopflow.sh "$@"` first in that script to keep loopflow base tooling aligned
- requires the `docker` CLI in `PATH` for repo image builds (`docker build`)
- reattaches to running agent containers after daemon restart

Current limitation: `fork` steps with `select: all` are not supported by the Docker executor yet.

## GitHub CI auto-fix

```bash
# Webhook target (GitHub check_run failures)
POST /v0/hooks/github

# One-shot poll for a single wave (requires github.token / LFD_GITHUB_TOKEN)
POST /v0/waves/{wave_id}/check-ci
```

Set `github.webhook_secret` (or `LFD_GITHUB_WEBHOOK_SECRET`) before enabling the webhook. `lfd` verifies `X-Hub-Signature-256` and ignores unsigned requests.
