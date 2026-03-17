# Remote deploy (EC2 + Docker + Caddy)

Run `lfd` remotely behind Caddy TLS.

## Prerequisites

- Ubuntu 22.04+ host (t3.medium/t4g.medium+)
- Docker + Docker Compose plugin
- Domain (or host name) pointing to the instance
- Security group allows inbound `443/tcp` and `80/tcp` (ACME challenge)
- Host already signed into studio (`~/.lf/credentials.json` present)

## Quick start

```bash
git clone https://github.com/loopflowstudio/loopflow.git
cd loopflow

export LFD_AUTH_MODE=studio
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
export LFD_AUTH_MODE=studio
export LF_DOMAIN='lfd.example.com'
export LF_TLS_MODE=internal   # internal (dev) or empty (public ACME)
export LFD_EXECUTOR_IMAGE='loopflow/agent:latest'
```

## Verify deployment

```bash
curl -f https://lfd.example.com/health
```

Then sign in through Concerto or `lfq` and connect to the remote daemon through the normal studio discovery flow.

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
- WS fails: ensure `/ws` is reachable through the same TLS host
- Remote clients cannot connect: verify `~/.lf/credentials.json` and successful studio registration logs on the host
