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

## Migrations

```bash
lfd migrate           # apply pending migrations for configured backend
lfd migrate --status  # print schema_version=<version>
```

`lfd migrate` uses the `mode` setting in `~/.lf/lfd.yaml` to choose backend (`sqlite` for native,
`postgres` for container). `LFD_DATABASE_URL` is required for postgres migrations.

## Install

```bash
lfd install          # install service for configured mode
lfd install --force  # tear down conflicting backend and reinstall
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
LFD_MODE          # optional mode override: native or container
LFD_HTTP_ADDR     # daemon listen address (default 127.0.0.1:2486)
LFD_DB_PATH       # sqlite path override (native mode)
LFD_DATABASE_URL  # required for container mode (postgres)
LFD_MAX_SLOTS     # concurrent run slots
LFD_AUTH_PROVIDER # local (default), static, or loopflow.studio
LFD_AUTH_TOKEN    # required when LFD_AUTH_PROVIDER=static
LFD_EXECUTOR_IMAGE # override agent image (default loopflow/agent:latest)
LFD_GITHUB_WEBHOOK_SECRET  # required for /v0/hooks/github signature verification
LFD_GITHUB_TOKEN           # optional; enables startup/on-demand CI polling
```

`lfd` reads `~/.lf/lfd.yaml` for daemon settings:

```yaml
mode: native  # native (default) or container

auth:
  provider: local # local (default), static, or loopflow.studio
  token: your-static-token # required when provider=static
  base_url: https://auth.loopflow.studio # used by loopflow.studio provider
executor:
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

`mode` (or `LFD_MODE`) selects a strict profile — `executor.type`, `storage`, `runtime_backend`, and
`service_manager` are all determined by the mode and cannot be overridden.

`credentials.mounts` uses named allowlisted mounts only:

- `claude` → `~/.claude`
- `codex` → `~/.codex`
- `gemini` → `~/.config/gemini`
- `gitconfig` → `~/.gitconfig`
- `ssh` → `~/.ssh`
- `gnupg` → `~/.gnupg`

Names are mounted read-only into the container. Raw `host:container` mount strings are rejected.

### Compose overrides

`lfd install` generates `~/.lf/docker-compose.yml` — don't edit it directly, it's regenerated on every install.

To customize the compose stack, create `~/.lf/docker-compose.override.yml`:

```yaml
# ~/.lf/docker-compose.override.yml
services:
  gateway:
    ports:
      - "3000:2486"   # expose on a different host port
    environment:
      - EXTRA_VAR=value
  postgres:
    command: postgres -c log_statement=all
```

Standard Docker Compose merge rules apply — the override file is layered on top of the managed file. `lfd` passes both files via `-f` flags when the override exists.

Environment overrides (non-identity fields only):

```bash
LFD_AUTH_PROVIDER=static
LFD_AUTH_TOKEN=your-static-token
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
