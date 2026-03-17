# Remote deploy (EC2 + Docker + Caddy)

Run `lfd` remotely behind Caddy TLS.

## Prerequisites

- Ubuntu 22.04+ host (t3.medium/t4g.medium+)
- Docker + Docker Compose plugin
- Domain or hostname pointing to the instance
- Security group allows inbound `443/tcp` and `80/tcp`
- Host already signed into studio (`~/.lf/credentials.json` present)

## Quick start

```bash
git clone https://github.com/loopflowstudio/loopflow.git
cd loopflow

export LF_DOMAIN='lfd.example.com'
export LF_TLS_MODE=internal   # unset for public ACME certs

docker compose -f docker/docker-compose.yml -f deploy/docker-compose.prod.yml up -d --build
```

Container mode and studio auth are already the default shape in these compose files.

## Verify

```bash
curl -f https://lfd.example.com/health
```

Then sign in through Concerto or `lfq` and connect through the normal studio discovery flow.

## Troubleshoot

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

Need agent credentials inside execution containers? Set `LFD_EXECUTOR_CREDENTIALS_MOUNTS=claude,ssh` in `.env` (the same config knob as `executor.credentials.mounts`) instead of editing raw compose volume lines.

Common failures:

- ACME cert not issued: DNS or port 80 is wrong
- WebSocket failures: make sure `/ws` is reachable through the same TLS host
- Remote clients cannot connect: verify `~/.lf/credentials.json` and studio registration logs on the host
