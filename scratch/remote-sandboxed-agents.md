# 01: Sandboxed Agent Execution

Sandbox agent step execution in `lfd` so host subprocesses are no longer the only runtime model.

## Current state

This branch shipped the execution abstraction and Docker backend:

- `AgentExecutor` with `LocalProcessExecutor` and `DockerExecutor`
- Executor selection from config/env (`executor.type`, `executor.image`, env overrides)
- Ephemeral Docker container per step with log streaming and explicit cleanup
- Executor-aware stop/delete/recovery termination paths
- Explicit credential injection (env + configured mounts) for Docker mode

Local execution remains the default.

## Image model

Two-layer image architecture:

**Base image** (`loopflow/agent:latest`) — we own, published from `docker/agent/Dockerfile`:
- All 4 coding agents (Claude Code, Codex, Gemini CLI, OpenCode)
- Git, curl, jq, core tools
- No toolchain opinions — that's the repo image's job

**Repo image** — lives in customer repo at `.lf/Dockerfile`, maintained by `_docker-gen` step:
- `FROM loopflow/agent:latest`
- Adds repo-specific toolchain (Rust, Python, Node, Go, etc.)
- Calls `.lf/env-setup.sh` for all setup

### env-setup.sh

Single idempotent script that installs everything the repo needs:
- Used by Dockerfile at build time (from scratch)
- Used by agents at runtime after adding dependencies (`update_env`)
- Package managers handle idempotency naturally

### Container security model

- Containers run as root (agents need `apt-get install`, `pip install`, etc.)
- Security boundary is the container itself, not the user inside it
- No `--privileged`, no host network, no Docker socket mount
- Credential mounts are explicit and configured

### Image caching

- Tag per wave: `lf-<wave-name>`
- Always run `docker build`, let Docker's layer cache handle freshness
- `.lf/.docker-stale` marker tells executor to rebuild before next wave
- **Future**: push to registry for hosted/remote execution

### Staleness detection

Post-commit hook (`.lf/hooks/docker-check`) runs `env-setup.sh` output comparison after every commit. If stale, touches `.lf/.docker-stale`. Executor checks this before starting a wave.

### Image selection

1. Repo has `.lf/Dockerfile` → build and use repo image
2. No `.lf/Dockerfile` → run `_docker-gen` in base image to create it, then build
3. Explicit `executor.image` in config → use that (BYO override)

## What remains to finish Phase 01

### 1) Repo-in-volume workspace model

Current Docker mode bind-mounts the active host worktree at `/workspace`.

Remaining work:
- Move clone/worktree lifecycle to Docker-managed per-repo volumes
- Mount only that repo volume into agent containers
- Ensure no broad host path mounts are required for normal operation

### 2) Durable container tracking

Current container IDs are in-memory only.

Remaining work:
- Persist active container IDs in run state/store
- Rehydrate terminate handles after daemon restart
- Verify stop/delete/recovery works across restarts

### 3) Credential mount UX

Current config uses raw `host_path:container_path` strings.

Remaining work:
- Decide whether to keep string syntax or switch to structured mount config
- Add clear validation/errors for malformed entries
- Keep deny-by-default behavior and read-only semantics

### 4) Runtime validation in live Docker environment

Current validation is unit tests + compile/test suite.

Remaining work:
- Add daemon-level end-to-end Docker execution test(s)
- Cover run, log streaming, cancellation, and cleanup behavior
- Validate behavior under concurrent waves

### 5) Repo image build pipeline

Executor needs to build `.lf/Dockerfile` before running steps in Docker mode.

Remaining work:
- Executor checks for `.lf/Dockerfile`, builds repo image if present
- Falls back to base image if no repo Dockerfile
- Runs `_docker-gen` to create `.lf/Dockerfile` when missing
- Checks `.lf/.docker-stale` to trigger rebuild between waves

### Build context

Respect `.dockerignore` if present. If no `.dockerignore` exists, fall back to `.gitignore`. Optimize later if needed.

## Open decisions

- Should credential mounts be promoted to a first-class typed config now, or deferred to Phase 02?

## Done criteria (Phase 01)

- Docker execution runs from Docker-managed repo volumes, not host worktree binds
- Cancellation/termination survives daemon restart (container tracking is durable)
- Credential config is explicit, validated, and least-privilege by default
- Live Docker end-to-end tests cover run + cancel + cleanup
- `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, and `cargo test --all` pass
