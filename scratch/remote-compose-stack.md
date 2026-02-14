# Docker Compose Stack

Package lfd + postgres into Docker Compose. Same compose file runs locally and on EC2.

## Problem

Phase 01 sandboxes agents in Docker containers, but lfd itself runs as a native process. Deploying remotely (Phase 04) means installing Rust, building lfd, managing postgres, configuring services — all by hand. There's no single artifact that captures "the entire lfd stack."

Containerizing lfd + postgres into Docker Compose gives us one deployment artifact. `docker compose up` on any machine with Docker starts the full stack. Test locally, deploy remotely with the same file.

## Approach

Three deliverables:

1. **`docker/lfd/Dockerfile`** — Multi-stage Rust build producing a minimal Debian image with `lfd` and `lf` binaries.
2. **`docker-compose.yml`** — lfd + postgres, Docker socket mounted, repos volume shared with agent containers.
3. **`lfd migrate` integration** — Compose runs migrations automatically on startup before serving.

No new Rust code. lfd already supports postgres storage, Docker executor, and env-var config. This is packaging, not implementation.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Nix/Guix packaging | Reproducible builds, no Docker required | Niche tooling, Docker already required for agent sandboxing |
| Static binary (musl) | No runtime image needed, just copy binary | Still need postgres, credentials, volumes — compose solves orchestration |
| Buildpack / Cloud Native Buildpack | Auto-detect Rust, build in cloud | Over-engineered for a single binary + postgres |
| Separate lfd + postgres containers, no compose | More manual but simpler | Compose is the standard; manual `docker run` commands are error-prone |

## Key decisions

**Docker-out-of-Docker, not Docker-in-Docker.** Mount the host Docker socket into the lfd container. Agent containers are siblings, not nested. This follows the wave roadmap's architecture: "lfd creates sibling containers via the Docker API." DinD adds complexity (privileged mode, storage drivers) for no benefit.

**Non-root lfd user inside the container, Docker group for socket access.** The lfd container runs as a non-root user (`lfd`, UID 1000). The Docker socket is owned by the `docker` group on the host. The Dockerfile adds the `lfd` user to a group matching the host's Docker socket GID. At runtime, compose passes `DOCKER_GID` (or we detect it). This avoids running the entire container as root while still allowing Docker API access.

Simpler alternative: just run as root. But defense-in-depth matters — if lfd has a vulnerability, root inside the container with the Docker socket mounted is effectively root on the host. Running as UID 1000 with only Docker group access limits blast radius.

**`LFD_HTTP_ADDR=0.0.0.0:2486` in compose, not in Dockerfile.** The Dockerfile doesn't set the bind address. Compose sets `LFD_HTTP_ADDR` to `0.0.0.0:2486` for container networking. Running lfd natively still defaults to `127.0.0.1:2486`. This keeps the image general-purpose.

**Auto-migrate on startup.** lfd already exits cleanly if postgres isn't ready. Add a compose `healthcheck` for lfd that hits `/health`. The startup sequence: postgres starts → compose waits for pg healthcheck → lfd starts → lfd runs `migrate` as part of startup → lfd serves. No separate `docker compose exec lfd lfd migrate` step.

This means lfd needs a small change: run migrations automatically when `LFD_STORAGE=postgres` before starting the HTTP server. Currently `lfd migrate` is a separate subcommand. We'll add auto-migration to the default startup path — if the schema is behind, migrate it forward. This is safe because migrations are idempotent and lfd is single-instance.

**No `repos:` named volume in compose.** The DockerExecutor already creates per-repo volumes (`lfd-repo-<hash>`) dynamically via the Docker API. A shared `repos:` volume in compose would conflict with this. The lfd container doesn't need a repos volume mounted — it creates them and mounts them into agent containers. The compose `repos:` volume from the original roadmap doc is wrong; removing it.

**`lfd-data` volume for `~/.lf/`.** Persists sqlite fallback DB, credentials, and config across container restarts.

**Credential mounts via `.env` and bind mounts.** Agent credentials (API keys) flow through env vars in `.env`. Subscription credentials (`.claude/`, `.ssh/`) are bind-mounted from the host into the lfd container, and lfd passes them through to agent containers. Compose `volumes:` section declares these bind mounts with `ro` flag.

## Scope

### In scope

- `docker/lfd/Dockerfile` — multi-stage build, non-root user, Docker socket compatible
- `docker-compose.yml` — lfd + postgres, health checks, env var config
- `.env.example` — template for credentials and config
- Auto-migration on lfd startup (small Rust change to `bin/lfd.rs`)
- Health check for lfd in compose (`/health` endpoint already exists, unauthenticated)
- Documentation: how to build and run

### Out of scope

- TLS termination (Phase 04 adds Caddy)
- Auth configuration (Phase 03 already shipped; compose just passes `LFD_AUTH_*` env vars)
- CI image builds (future — build locally for now)
- Multi-arch images (build on target arch; cross-compile is a separate concern)
- Cursor file access strategy (Phase 06)

## Implementation

### Dockerfile.lfd

```dockerfile
FROM rust:1.82-bookworm AS builder
WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY rust ./rust
RUN cargo build -p loopflow --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends \
      ca-certificates libssl3 git \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /build/target/release/lf /usr/local/bin/lf
COPY --from=builder /build/target/release/lfd /usr/local/bin/lfd

# Non-root user. Docker GID set at runtime via entrypoint.
RUN useradd -m -s /bin/bash -u 1000 lfd
COPY docker/lfd/entrypoint.sh /usr/local/bin/entrypoint.sh
RUN chmod +x /usr/local/bin/entrypoint.sh

USER lfd
WORKDIR /home/lfd
RUN mkdir -p /home/lfd/.lf

EXPOSE 2486
ENTRYPOINT ["entrypoint.sh"]
CMD ["lfd", "run"]
```

### entrypoint.sh

Handles Docker GID assignment at runtime:

```bash
#!/bin/bash
set -e

# If Docker socket exists and DOCKER_GID is set, ensure lfd can access it.
if [ -S /var/run/docker.sock ] && [ -n "$DOCKER_GID" ]; then
    # Create docker group with matching GID if it doesn't exist.
    if ! getent group "$DOCKER_GID" >/dev/null 2>&1; then
        groupadd -g "$DOCKER_GID" docker 2>/dev/null || true
    fi
    GROUP_NAME=$(getent group "$DOCKER_GID" | cut -d: -f1)
    usermod -aG "$GROUP_NAME" lfd 2>/dev/null || true
fi

exec "$@"
```

Wait — `usermod` and `groupadd` require root. But `USER lfd` in Dockerfile means entrypoint runs as non-root. Two options:

**Option A: Run entrypoint as root, `exec gosu lfd "$@"`.** Standard pattern (postgres, redis images all do this). Requires installing `gosu` in the image.

**Option B: Skip GID dance, run as root.** Simpler. Agent containers are the security boundary, not the lfd container. lfd already has the Docker socket — it can do anything Docker can. Running as UID 1000 with Docker socket access is security theater.

**Decision: Option B. Run lfd as root inside the container.** The Docker socket grants root-equivalent access regardless of UID. Pretending otherwise adds complexity. The entrypoint becomes trivial. Agent containers (Phase 01) are the real security boundary.

Revised Dockerfile:

```dockerfile
FROM rust:1.82-bookworm AS builder
WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY rust ./rust
RUN cargo build -p loopflow --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends \
      ca-certificates libssl3 git \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /build/target/release/lf /usr/local/bin/lf
COPY --from=builder /build/target/release/lfd /usr/local/bin/lfd

RUN mkdir -p /root/.lf
WORKDIR /root

EXPOSE 2486
ENTRYPOINT ["lfd"]
CMD ["run"]
```

No entrypoint script. No user management. `lfd run` falls through to the serve path in the existing binary.

### docker-compose.yml

```yaml
services:
  lfd:
    build:
      context: .
      dockerfile: docker/lfd/Dockerfile
    ports:
      - "${LFD_PORT:-2486}:2486"
    environment:
      LFD_HTTP_ADDR: "0.0.0.0:2486"
      LFD_STORAGE: postgres
      LFD_DATABASE_URL: "postgres://lfd:lfd@postgres:5432/lfd"
      LFD_EXECUTOR_TYPE: docker
      LFD_EXECUTOR_IMAGE: "${LFD_EXECUTOR_IMAGE:-loopflow/agent:latest}"
      LFD_AUTH_PROVIDER: "${LFD_AUTH_PROVIDER:-local}"
      LFD_AUTH_TOKEN: "${LFD_AUTH_TOKEN:-}"
    env_file:
      - path: .env
        required: false
    volumes:
      - /var/run/docker.sock:/var/run/docker.sock
      - lfd-data:/root/.lf
    depends_on:
      postgres:
        condition: service_healthy
    healthcheck:
      test: ["CMD", "curl", "-sf", "http://localhost:2486/health"]
      interval: 10s
      timeout: 5s
      retries: 3
      start_period: 30s
    restart: unless-stopped

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
    restart: unless-stopped

volumes:
  pgdata:
  lfd-data:
```

Notes:
- No `repos:` volume — DockerExecutor creates per-repo volumes dynamically.
- `curl` for healthcheck — need to install `curl` in the lfd image (add to `apt-get install`).
- `.env` file optional — compose handles missing `.env` gracefully with `required: false`.
- Credential env vars (`ANTHROPIC_API_KEY`, `GH_TOKEN`, etc.) flow through `.env` → lfd → agent containers via DockerExecutor's `collect_env()`.

### .env.example

```bash
# Agent credentials (passed through to agent containers)
# ANTHROPIC_API_KEY=sk-ant-...
# OPENAI_API_KEY=sk-...
# GEMINI_API_KEY=...
# GH_TOKEN=ghp_...

# Auth (Phase 03 — static token for remote access)
# LFD_AUTH_PROVIDER=static
# LFD_AUTH_TOKEN=<generate with: openssl rand -hex 32>

# Agent image override
# LFD_EXECUTOR_IMAGE=loopflow/agent:latest
```

### Auto-migration (Rust change)

In `rust/loopflow/src/bin/lfd.rs`, add auto-migration when storage is postgres:

```rust
// After store creation, before serving:
if storage == "postgres" {
    let url = std::env::var("LFD_DATABASE_URL").expect("...");
    let version = PostgresStore::migrate_async(&url).await?;
    tracing::info!(schema_version = version, "postgres schema up to date");
}
```

This runs every startup. `migrate_async` is idempotent — if schema is current, it's a no-op (one query to check version).

### Credential bind mounts (optional)

For subscription auth (Claude Max, Codex ChatGPT), bind-mount credential directories from host:

```yaml
# Add to lfd service volumes (uncomment as needed):
#   - ${HOME}/.claude:/root/.claude:ro
#   - ${HOME}/.codex:/root/.codex:ro
#   - ${HOME}/.ssh:/root/.ssh:ro
#   - ${HOME}/.gitconfig:/root/.gitconfig:ro
```

These are commented out in compose by default. Document in `.env.example`.

## Build and run

```bash
# Build agent image first (Phase 01 prerequisite)
docker build -t loopflow/agent:latest -f docker/agent/Dockerfile docker/agent/

# Build and start the stack
docker compose up --build

# Verify
curl http://localhost:2486/health

# Add a repo and create a wave (via lfq or curl)
lfq create mywave /path/to/repo
lfq run mywave
lfq logs mywave
```

## Risks

**Docker socket GID mismatch.** On Linux, the Docker socket is typically `root:docker` (GID varies). Running as root inside the container avoids this entirely. On macOS with Docker Desktop/OrbStack, the socket is accessible regardless of GID.

**Volume permission conflicts between lfd and agent containers.** lfd runs as root (UID 0). Agent containers may run as a different UID. Both write to the same repo volumes. Root can always write; the agent image's user needs to be able to read/write too. The existing agent Dockerfile doesn't set a USER, so agents also run as root inside their containers. No conflict.

**Cargo build time in Docker.** Clean builds take 5–10 minutes. Docker layer caching helps for incremental builds, but a Cargo.toml change invalidates the cache. Mitigation: use `cargo-chef` for dependency caching in CI. For local dev, accept the build time or build natively and volume-mount the binary.

**Postgres data loss on `docker compose down -v`.** Named volumes persist across `docker compose down` but are destroyed by `down -v`. Document this. For dev, it's fine — waves are ephemeral. For remote (Phase 04), this is the only state that matters.

## Done when

- `docker compose up --build` starts lfd + postgres
- `curl http://localhost:2486/health` returns OK
- lfd in container creates agent containers via Docker socket
- Postgres stores wave state (auto-migrated on startup)
- Agent containers access dynamically-created repo volumes
- Logs stream from agent containers through lfd to API consumers
- Same compose file works on macOS (Docker Desktop/OrbStack) and Linux
- `.env.example` documents all configurable variables
