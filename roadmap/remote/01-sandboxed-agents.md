# 01: Sandboxed Agent Execution

Run agents in Docker containers with controlled filesystem, network, and credentials. lfd orchestrates containers instead of forking local processes.

## What exists after this

lfd spawns each wave's agent in a Docker container. Repos live in Docker volumes, not on the host. Agents can't access the host filesystem, can't interfere with each other, and only get the credentials they need. lfd itself still runs as a native process.

## Why containerize

Security. Agents run arbitrary code — file edits, bash commands, git operations. A container limits the blast radius:

- No access to host filesystem (only the repo volume)
- Explicit credentials (env vars or mounted credential files, read-only)
- Network restricted to outbound (API calls, git push)
- One container per wave — waves can't interfere with each other

## Architecture

```
lfd (native process, has Docker socket access)
  ├── manages wave state, serves API to Concerto
  ├── repos volume mounted (for git worktree operations)
  └── creates agent containers via Docker API

wave-1 (container, created by lfd at runtime)
  ├── repos volume (shared, read-write)
  ├── agent CLI (claude/codex/gemini/opencode)
  ├── credentials (env vars + mounted cred files ro)
  └── network: outbound only

wave-2 (container, created by lfd at runtime)
  └── (same pattern)
```

lfd creates sibling containers via the Docker API (bollard crate). Repo volumes are shared between lfd and agent containers. Agents never touch the host filesystem directly.

## Agent image

One image with all supported agents pre-installed:

```dockerfile
# Dockerfile.agent
FROM node:22-bookworm-slim

# System dependencies
RUN apt-get update && apt-get install -y --no-install-recommends \
    git curl openssh-client ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Agent CLIs
RUN npm install -g @anthropic-ai/claude-code @openai/codex @google/gemini-cli

# opencode (Go binary)
RUN curl -fsSL https://raw.githubusercontent.com/opencode-ai/opencode/refs/heads/main/install | bash

# lf binary (for context assembly)
RUN curl -fsSL https://github.com/loopflowstudio/loopflow/releases/latest/download/install.sh | sh

# Non-root user
RUN useradd -m -s /bin/bash agent
USER agent
WORKDIR /home/agent
```

### Agent headless commands

All 4 agents support non-interactive execution with API key auth:

| Agent | Command | Permission bypass |
|-------|---------|-------------------|
| Claude Code | `claude -p "prompt" --allowedTools "Bash,Read,Edit"` | `--dangerously-skip-permissions` |
| Codex CLI | `codex exec "prompt"` | `--full-auto` |
| Gemini CLI | `gemini -p "prompt"` | `--yolo` |
| opencode | `opencode run "prompt"` | Auto in non-interactive |

None require a TTY. All authenticate via env vars.

## Executor trait

Replace lfd's current `fork+exec` spawning with a trait that supports both local process and container backends:

```rust
#[async_trait]
pub trait AgentExecutor: Send + Sync {
    async fn spawn(&self, config: AgentSpawnConfig) -> Result<AgentHandle>;
    async fn status(&self, handle: &AgentHandle) -> Result<AgentStatus>;
    async fn terminate(&self, handle: &AgentHandle) -> Result<()>;
    async fn logs(&self, handle: &AgentHandle) -> Result<LogStream>;
    async fn wait(&self, handle: &AgentHandle) -> Result<i32>;
}

pub struct AgentSpawnConfig {
    pub agent: AgentType,          // claude, codex, gemini, opencode
    pub command: Vec<String>,      // headless command + args
    pub working_dir: PathBuf,      // worktree path inside container
    pub env: HashMap<String, String>,  // API keys, git tokens
    pub repo_volume: String,       // Docker volume name
}
```

### Backends

| Backend | When | How agents run |
|---------|------|---------------|
| `LocalProcess` | Default (backwards compat) | Fork + exec, current behavior |
| `DockerExecutor` | Container mode | Docker API via bollard crate |

### Configuration

```yaml
# ~/.lf/lfd.yaml
executor:
  type: local          # default: fork+exec (no Docker)
  # type: docker
  # image: loopflow/agent:latest
```

### Docker executor implementation

```rust
use bollard::Docker;

pub struct DockerExecutor {
    docker: Docker,
    image: String,
}

impl AgentExecutor for DockerExecutor {
    async fn spawn(&self, config: AgentSpawnConfig) -> Result<AgentHandle> {
        let container = self.docker.create_container(
            None,
            bollard::container::Config {
                image: Some(self.image.clone()),
                cmd: Some(config.command),
                working_dir: Some(config.working_dir.to_string_lossy().into()),
                env: Some(config.env.iter()
                    .map(|(k, v)| format!("{k}={v}"))
                    .collect()),
                host_config: Some(bollard::models::HostConfig {
                    binds: Some(vec![
                        format!("{}:/repos", config.repo_volume),
                    ]),
                    network_mode: Some("bridge".to_string()),
                    ..Default::default()
                }),
                ..Default::default()
            },
        ).await?;

        self.docker.start_container(&container.id, None).await?;

        Ok(AgentHandle { id: container.id })
    }

    async fn logs(&self, handle: &AgentHandle) -> Result<LogStream> {
        let stream = self.docker.logs(&handle.id, Some(LogsOptions {
            follow: true,
            stdout: true,
            stderr: true,
            ..Default::default()
        }));
        Ok(LogStream::from_docker(stream))
    }

    async fn terminate(&self, handle: &AgentHandle) -> Result<()> {
        self.docker.stop_container(&handle.id, None).await?;
        self.docker.remove_container(&handle.id, None).await?;
        Ok(())
    }
}
```

## Repo volumes

One Docker volume per repo. Clone once when the repo is added to loopflow. Worktrees are siblings inside the same volume.

```
Docker volume: repo-loopflow
  /repos/loopflow/                # main clone
  /repos/loopflow.wave-feature/   # worktree for wave
  /repos/loopflow.wave-bugfix/    # worktree for wave
```

### Lifecycle

1. **Add repo**: `lf repo add git@github.com:you/repo.git`
   - lfd creates a Docker volume
   - Clones the repo into the volume
2. **Create wave**: lfd creates a worktree inside the volume (git worktree add)
3. **Run wave**: lfd spawns agent container, mounts the volume at `/repos/`
4. **Stop wave**: container stops, volume persists
5. **Delete wave**: lfd removes the worktree from the volume

The volume persists across container stop/start. Git state (branches, worktrees, objects) is preserved.

## Credentials per wave

Each wave container gets only what it needs:

| Credential | How | Scope |
|-----------|-----|-------|
| Claude Code (Max/Pro) | `~/.claude/` mounted read-only | Global |
| Claude Code (API) | `ANTHROPIC_API_KEY` env var | Per wave config |
| Codex CLI (ChatGPT) | `~/.codex/auth.json` mounted read-only | Global |
| Codex CLI (API) | `CODEX_API_KEY` env var | Per wave config |
| Gemini CLI | `GEMINI_API_KEY` env var or Google OAuth creds mounted | Per wave or global |
| opencode | Provider-specific env var or creds | Per wave config |
| Git push access | `GH_TOKEN` env var for HTTPS | Shared or per wave |
| Git config | `~/.gitconfig` mounted read-only | Global |

Default to subscription auth (mounted credential files, read-only). The user explicitly grants loopflow access to these credentials. API key env vars are the alternative for users who prefer API billing or don't have subscriptions.

Read-only mounts of auth tokens are acceptable — these aren't arbitrary host directories, they're credentials the user has chosen to share with loopflow.

## Open questions

- **Bollard vs shelling out to `docker`**: bollard is a pure Rust Docker client. Alternative: just exec `docker run/stop/logs`. Bollard is cleaner but adds a dependency.
- **Container lifecycle**: does the container stay running between steps, or does each step get a fresh container? Keeping it running is simpler (no re-clone, warm caches). But a fresh container per step is more isolated.
- **Network policy**: "outbound only" is the Docker default (bridge mode). Do we need tighter restrictions (allowlist specific domains)?
- **Image size**: the all-agents image will be large (Node.js + Go + Rust artifacts). Consider a base image with just system deps, and install agents via volume mount or at container start.
- **Multi-arch**: need ARM images for Apple Silicon Macs and t4g EC2. Docker buildx handles this.

## Done when

- Agent Docker image builds with Claude Code, Codex, Gemini CLI, opencode
- `AgentExecutor` trait implemented with `LocalProcess` and `DockerExecutor` backends
- lfd can spawn a wave agent in a Docker container
- Agent executes Claude Code headless, produces commits
- Logs stream from container to lfd to Concerto
- Repo volume persists across container stop/start
- Container has no access to host filesystem (only repo volume)
- `executor.type: docker` in config switches to container mode
- `executor.type: local` still works (no regression)
