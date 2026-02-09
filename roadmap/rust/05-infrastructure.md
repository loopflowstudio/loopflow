# 05: Infrastructure

Executor abstraction + deployment packaging for running lfd remotely.

## Executor Abstraction

Phase 1 lfd spawns agents as local processes. For remote deployments, abstract the execution backend.

### Trait

```rust
#[async_trait]
pub trait AgentExecutor: Send + Sync {
    async fn spawn(&self, config: AgentSpawnConfig) -> Result<AgentHandle>;
    async fn status(&self, handle: &AgentHandle) -> Result<AgentStatus>;
    async fn terminate(&self, handle: &AgentHandle) -> Result<()>;
    async fn logs(&self, handle: &AgentHandle) -> Result<LogStream>;
    async fn wait(&self, handle: &AgentHandle) -> Result<i32>;
}
```

### Backends

| Executor | How agents run | Credentials | Use case |
|----------|---------------|-------------|----------|
| `LocalProcess` | Fork + exec | `~/.claude` | Phase 1 default |
| `Container` | Docker containers | Mounted volume | Self-hosted isolation |
| `Kubernetes` | K8s Jobs | Secret volume mount | Production/hosted |

### Configuration

```yaml
# ~/.lf/lfd.yaml
executor:
  type: local                    # default
  # type: container
  # type: kubernetes
```

## Container Images

### loopflow/lfd

```dockerfile
FROM rust:1.93-bookworm AS builder
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY rust ./rust
RUN cargo build -p loopflow --release --bin lfd

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates libssl3 \
    && rm -rf /var/lib/apt/lists/*
RUN useradd -m -s /bin/bash lfd
USER lfd
COPY --from=builder /app/target/release/lfd /usr/local/bin/lfd
EXPOSE 8080
ENTRYPOINT ["lfd"]
```

### loopflow/agent

Minimal image with Claude CLI (and optionally Codex/Gemini CLI).

## Docker Compose (Self-Hosted)

```yaml
services:
  lfd:
    image: loopflow/lfd:latest
    environment:
      LFD_HTTP_ADDR: 0.0.0.0:8080
      LFD_AUTH_PROVIDER: loopflow.studio
      LFD_AUTH_ALLOWED_USERS: ${ALLOWED_USERS}
      LFD_EXECUTOR_TYPE: container
      LFD_EXECUTOR_IMAGE: loopflow/agent:latest
    volumes:
      - /var/run/docker.sock:/var/run/docker.sock
      - ${CLAUDE_CREDENTIALS:-~/.claude}:/claude-credentials:ro
    ports:
      - "8080:8080"

  # Optional: use postgres instead of SQLite
  # postgres:
  #   image: postgres:16
  #   environment:
  #     POSTGRES_USER: lfd
  #     POSTGRES_PASSWORD: ${POSTGRES_PASSWORD}
  #     POSTGRES_DB: lfd
```

## Helm Chart

For Kubernetes deployment. Key resources:

- Deployment (lfd)
- Service (HTTP port 8080)
- Ingress with TLS (cert-manager)
- ServiceAccount + RBAC (for creating agent Jobs)
- Secrets (Claude credentials, postgres)

```bash
helm install loopflow loopflow/loopflow \
  --namespace loopflow \
  --set lfd.auth.allowedUsers="{user_abc123}" \
  --set ingress.hosts[0].host=lfd.example.com
```

## Done When

- [ ] `AgentExecutor` trait defined with local/container/k8s implementations
- [ ] Container executor mounts credentials, streams logs
- [ ] K8s executor creates Jobs, manages lifecycle
- [ ] `loopflow/lfd` and `loopflow/agent` images build and publish
- [ ] Docker Compose deploys lfd with container executor
- [ ] Helm chart deploys to Kubernetes with RBAC for Jobs
- [ ] Health checks work through ingress

## Dependencies

- Requires: 04-auth
- Enables: 06-hosted
