# 02: Docker Compose Stack

Package lfd + postgres into Docker Compose for deployment. Same compose file runs locally and on EC2.

## What exists after this

`docker compose up` starts lfd + postgres in containers. lfd creates agent containers via the Docker socket (Docker-out-of-Docker). The compose file is the deployment artifact — same one runs on your Mac and on EC2 (Phase 04).

## Why containerize lfd

Phase 01 sandboxes agents. This phase packages the whole stack:

- **Reproducible deployment**: `docker compose up` on any machine with Docker
- **Postgres included**: No separate database install
- **Same artifact locally and remote**: Test the exact deployment on your Mac before pushing to EC2

## Architecture

```
docker compose up
  ├── lfd (container, Docker socket mounted)
  │     ├── manages wave state, serves API to Concerto
  │     ├── repos volume mounted (for git worktree operations)
  │     └── creates agent containers via Docker API
  │
  ├── postgres (container): wave state
  │
  ├── wave-1 (container, created by lfd at runtime)
  │     ├── repos volume (shared, read-write)
  │     ├── agent CLI (claude/codex/gemini/opencode)
  │     ├── credentials (env vars + mounted cred files ro)
  │     └── network: outbound only
  │
  └── wave-2 (container, created by lfd at runtime)
        └── (same pattern)
```

Agent containers are created dynamically by lfd (Phase 01's DockerExecutor). The compose file only defines lfd + postgres.

## lfd image

```dockerfile
# Dockerfile.lfd
FROM rust:1.82-bookworm AS builder
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY rust ./rust
RUN cargo build -p loopflow --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates libssl3 git \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/lf /usr/local/bin/lf
COPY --from=builder /app/target/release/lfd /usr/local/bin/lfd

RUN useradd -m -s /bin/bash lfd
USER lfd
WORKDIR /home/lfd
RUN mkdir -p /home/lfd/.lf

EXPOSE 2486
ENTRYPOINT ["lfd", "run"]
```

## Docker Compose

```yaml
services:
  lfd:
    build:
      context: .
      dockerfile: Dockerfile.lfd
    ports:
      - "2486:2486"
    environment:
      LFD_HOST: "0.0.0.0"
      LFD_PORT: "2486"
      LFD_STORAGE: postgres
      LFD_DATABASE_URL: "postgres://lfd:lfd@postgres:5432/lfd"
      LFD_EXECUTOR_TYPE: docker
      LFD_EXECUTOR_IMAGE: "loopflow/agent:latest"
    volumes:
      - /var/run/docker.sock:/var/run/docker.sock
      - repos:/repos
      - lfd-data:/home/lfd/.lf
    depends_on:
      postgres:
        condition: service_healthy

  postgres:
    image: postgres:16-alpine
    environment:
      POSTGRES_USER: lfd
      POSTGRES_PASSWORD: lfd
      POSTGRES_DB: lfd
    volumes:
      - pgdata:/var/lib/postgresql/data
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U lfd"]
      interval: 5s
      timeout: 5s
      retries: 5

volumes:
  repos:
  pgdata:
  lfd-data:
```

## Cursor file access

Files live inside Docker volumes, not on the host. Two options for editing:

**Option A: Cursor "Attach to Container"**
```bash
cursor --remote attached-container+<container-id> /repos/loopflow.wave-feature
```
No SSH needed. Uses Docker socket. Concerto constructs this command from the wave's container ID and worktree path.

**Option B: Mount volume on host at known path**
```bash
# lfd mounts the volume at a predictable host path for Cursor
docker run -v repo-loopflow:/repos -v /tmp/lf-repos/loopflow:/host-repos ...
```
Then Cursor opens `/tmp/lf-repos/loopflow/loopflow.wave-feature`.

Option A is cleaner (no host paths) but requires Docker Desktop or OrbStack. Option B works everywhere but adds host filesystem coupling.

## Constraints

- **Docker socket access**: lfd container needs Docker socket mounted. Standard Docker-out-of-Docker pattern.
- **Postgres password**: Hardcoded `lfd/lfd` for dev. Use secrets for production.
- **Volume permissions**: lfd and agent containers need compatible UIDs for the repos volume.

## Done when

- `docker compose up` starts lfd + postgres
- lfd in container creates agent containers via Docker socket
- Postgres stores wave state (existing postgres backend, no new code)
- `curl http://localhost:2486/health` returns OK
- Agent containers access repos volume
- Logs stream from agent containers through lfd
- Same compose file works on macOS (Docker Desktop/OrbStack) and Linux
