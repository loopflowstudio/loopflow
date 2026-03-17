# Remote deploy (EC2 + Docker + Caddy)

Run lfd remotely behind Caddy TLS and verify it from your laptop.

## Prerequisites

- Ubuntu 22.04+ host (t3.medium/t4g.medium+)
- Docker + Docker Compose plugin
- Domain (or host name) pointing to the instance
- Security group allows inbound `443/tcp` and `80/tcp` (ACME challenge)
- CI auth token for remote auth (`LFD_AUTH_TOKEN`)

## Quick start

```bash
git clone https://github.com/loopflowstudio/loopflow.git
cd loopflow

export LFD_AUTH_PROVIDER=ci
export LFD_AUTH_TOKEN='<strong-random-token>'
export LF_DOMAIN='lfd.example.com'

# Dev/internal CA mode (self-signed by Caddy local CA)
export LF_TLS_MODE=internal

# For public ACME certs, unset LF_TLS_MODE (or set it to empty)
# unset LF_TLS_MODE

# Optional: executor image
export LFD_EXECUTOR_IMAGE='loopflow/agent:latest'

docker compose -f docker/docker-compose.yml -f deploy/docker-compose.prod.yml up -d --build
```

## Configuration

```bash
# Required for remote auth
export LFD_AUTH_PROVIDER=ci
export LFD_AUTH_TOKEN='<token>'

# Caddy domain and TLS mode
export LF_DOMAIN='lfd.example.com'
export LF_TLS_MODE=internal   # internal (dev) or empty (public ACME)

# Optional executor image override
export LFD_EXECUTOR_IMAGE='loopflow/agent:latest'
```

## Verify deployment

```bash
uv run python scripts/test_remote_smoke.py \
  --url https://lfd.example.com \
  --token "$LFD_AUTH_TOKEN" \
  --repo /absolute/path/to/loopflow/on/remote
```

On fresh hosts with no existing waves, `--repo` is required. Once waves exist, the script can default to the first `/v0/repos` entry.

For internal-CA deployments, pass custom trust or run insecure:

```bash
uv run python scripts/test_remote_smoke.py \
  --url https://lfd.example.com \
  --token "$LFD_AUTH_TOKEN" \
  --ca-cert /path/to/caddy-local-root.crt

# or
uv run python scripts/test_remote_smoke.py \
  --url https://lfd.example.com \
  --token "$LFD_AUTH_TOKEN" \
  --insecure
```

## Credential mounts for agent execution

Edit `docker/docker-compose.yml` and mount only credentials you intend to use:

```yaml
volumes:
  - ${HOME}/.claude:/root/.claude:ro
  - ${HOME}/.codex:/root/.codex:ro
  - ${HOME}/.ssh:/root/.ssh:ro
  - ${HOME}/.gitconfig:/root/.gitconfig:ro
```

Then redeploy compose.

## Troubleshooting

```bash
# Service status
docker compose -f docker/docker-compose.yml -f deploy/docker-compose.prod.yml ps

# lfd health
curl -f https://lfd.example.com/health

# Caddy logs
docker compose -f docker/docker-compose.yml -f deploy/docker-compose.prod.yml logs --tail 200 caddy

# lfd logs
docker compose -f docker/docker-compose.yml -f deploy/docker-compose.prod.yml logs --tail 200 lfd
```

Common failures:

- ACME cert not issued: domain DNS or port 80 blocked
- `401 missing token`: wrong/missing `Authorization: Bearer <token>`
- SSE appears delayed: ensure `flush_interval -1` is in `deploy/Caddyfile`
- WS fails: ensure `/ws` is reachable through the same TLS host and token auth

## Manual operator checks (not in smoke script)

- Connect Concerto in Remote mode to `https://<domain>` with token
- Open remote editor for a wave worktree
- Open remote terminal for a wave worktree
- Restart lfd container and verify reconnect behavior in Concerto
