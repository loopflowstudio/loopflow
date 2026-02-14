# EC2 Infrastructure

Deploy the containerized lfd stack to an ARM EC2 instance with TLS. Same compose file tested locally now runs remotely.

## Problem

Phases 01-03 built everything needed to run lfd remotely: Docker executor, compose stack, static token auth. But it only runs on localhost. There's no remote machine to point Concerto at. Phase 04 creates that machine.

The user needs a single EC2 instance — not a cluster, not HA, not auto-scaling. A dev box that runs `docker compose up` and serves HTTPS.

## Approach

Split into two scopes: **Terraform** (in the studio repo, creates the EC2 instance) and **deployment artifacts** (in this repo, files that get copied to the instance). This repo provides the Caddyfile for TLS and a deploy script. Terraform lives in studio because it manages infrastructure across multiple projects.

### Terraform (studio repo)

- `studio/terraform/dev/main.tf` — EC2 t4g.medium, 50GB gp3, Ubuntu 24.04 ARM
- `studio/terraform/dev/setup.sh` — user_data script installs Docker, creates `lfd` user
- Security group: SSH + HTTPS from operator IP only. All egress open.
- Elastic IP for stable addressing.

### Deployment artifacts (this repo)

**Caddyfile** — TLS reverse proxy. Self-signed cert via `tls internal`. Added to compose as a service.

```
# Caddyfile
:443 {
  tls internal
  reverse_proxy lfd:2486
}
```

**docker-compose.prod.yml** — Override file for remote deployment. Adds Caddy, removes host port mapping on lfd (Caddy handles external traffic), sets auth to static.

```yaml
services:
  caddy:
    image: caddy:2-alpine
    ports:
      - "443:443"
    volumes:
      - ./Caddyfile:/etc/caddy/Caddyfile
      - caddy-data:/data
    depends_on:
      - lfd
    restart: unless-stopped

  lfd:
    ports: !reset []  # Caddy handles external traffic
    environment:
      LFD_AUTH_PROVIDER: static

volumes:
  caddy-data:
```

Usage: `docker compose -f docker-compose.yml -f docker-compose.prod.yml up -d`

**deploy.sh** — Script that copies artifacts and starts the stack. Not automated infrastructure — a script you run from your laptop.

```bash
#!/bin/bash
set -euo pipefail
HOST="${1:-lfd-dev}"

scp docker-compose.yml docker-compose.prod.yml Caddyfile .env "$HOST":~/
ssh "$HOST" 'docker compose -f docker-compose.yml -f docker-compose.prod.yml up -d'
ssh "$HOST" 'docker compose ps'
```

**Updated .env.example** — Add Caddy-relevant vars and clarify remote requirements.

### Build strategy: build on the instance

The Dockerfile uses `rust:1.82-bookworm` and `debian:bookworm-slim` — both multi-arch. Building on the t4g (ARM) instance produces native ARM images without cross-compilation. Clean build takes 5-10 minutes. Acceptable for infrequent deploys.

Alternative: cross-compile on Mac (Apple Silicon → ARM Linux). Faster iteration but adds `cross` toolchain dependency and musl/glibc complexity. Not worth it for a dev box. Revisit if deploy frequency increases.

### Agent image on ARM

`docker/agent/Dockerfile` uses `node:22-bookworm-slim` — multi-arch, works on ARM. The `install-loopflow.sh` script needs to pull ARM binaries. Check that the install script and agent CLI tools (Claude Code, Codex, Gemini CLI) have ARM builds. Node-based tools (npm packages) work anywhere Node runs. Go binaries (opencode) need ARM builds.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| ECS Fargate | Managed containers, no EC2 | Docker-in-Docker problem — lfd creates sibling containers via Docker socket. Fargate doesn't expose Docker socket. |
| Fly.io | Simple deploys, built-in TLS | Same Docker socket issue. Also adds vendor dependency for a dev box. |
| Lambda + container | Pay-per-use | Long-running daemon with WebSocket connections. Lambda's 15-min limit and cold starts are deal-breakers. |
| Docker context (remote Docker API) | Build locally, deploy remotely | Exposes Docker API over the network. Security nightmare. And doesn't solve the "need a machine" problem. |
| Lightsail | Simpler than EC2 | No ARM instances. More expensive for equivalent specs. Less control over security groups. |
| Build on Mac, push to registry | Faster iteration | Requires a container registry (ECR/GHCR). Cross-arch images need buildx. Adds complexity for infrequent deploys. |

## Key decisions

**Docker-out-of-Docker, not DinD.** lfd creates agent containers as siblings via the host Docker socket. This is fundamental to Phase 01's architecture. It rules out any deployment target that doesn't give us a Docker socket (Fargate, Lambda, Fly.io). EC2 with Docker installed is the simplest option that works.

> Remote roadmap principle: "lfd is already the remote server. Concerto is already a thin client. The protocol doesn't change — only the host and transport."

**t4g.medium (ARM).** 2 vCPU, 4GB RAM, ~$25/mo. ARM because it's cheaper and our Mac builds are already ARM. All Docker base images are multi-arch. If 4GB is tight with multiple agent containers running, move to t4g.large (8GB, ~$50/mo) — vertical scaling, no architecture change.

**Self-signed TLS via Caddy.** No domain needed. Caddy's `tls internal` generates a CA and cert on first start. Concerto pins the cert fingerprint on first connect (TOFU — trust on first use). Good enough for a single-user dev box. Phase 07 (Studio Auth) may bring a real domain and Let's Encrypt, but that's future work.

> Phase 04 constraint: "Self-signed TLS: Caddy generates internal certs. Concerto pins the cert fingerprint on first connect."

**IP-restricted security group.** SSH and HTTPS only from the operator's IP. This is the primary security boundary for Phase 04. Combined with static token auth (Phase 03), it's defense in depth: network-level restriction + application-level auth.

**Semi-manual deploy, not CI/CD.** A shell script, not a pipeline. This is a dev box. Deploy frequency is low (weekly at most). The deploy script is ~5 lines: scp files, docker compose up. Automating further adds complexity without proportional value.

**Compose override file, not a separate compose.** `docker-compose.prod.yml` extends the base `docker-compose.yml`. Same services, same health checks, same auto-migration. Caddy and auth configuration are the only additions. Local dev uses the base file. Remote uses base + prod override.

**Build on the instance, not cross-compile.** The t4g has 2 vCPU — clean Rust builds take 5-10 minutes. Rebuilds with Docker layer cache are faster. Cross-compilation from Mac adds `cross`, musl vs glibc decisions, and toolchain maintenance. For infrequent deploys, building on-instance is simpler and produces native binaries without any cross-compilation risk.

## Scope

### In scope
- Terraform for EC2 + security group + EIP (in studio repo)
- User_data script to install Docker
- Caddyfile for TLS termination
- docker-compose.prod.yml override
- deploy.sh for copying artifacts and starting the stack
- Updated .env.example with remote-specific guidance
- SSH config documentation

### Out of scope
- CI/CD pipeline for deploys
- Multi-arch Docker image registry
- Cargo-chef dependency caching in Dockerfile
- Monitoring/alerting (CloudWatch, Datadog, etc.)
- Backup/restore for postgres data
- Domain name / Let's Encrypt (Phase 07)
- Auto-scaling or load balancing
- Concerto changes (Phase 05)

## Done when

- `terraform apply` in studio repo creates the EC2 instance with Docker installed
- `./deploy.sh lfd-dev` copies compose files and starts the stack
- `curl -k https://<elastic-ip>/health` returns OK through Caddy TLS
- `curl -k -H "Authorization: Bearer $TOKEN" https://<elastic-ip>/v0/waves` returns wave list
- Agent image builds on ARM instance
- A wave can run remotely (agent in container on EC2)
- The same `docker-compose.yml` works locally (without prod override) and remotely (with prod override)
