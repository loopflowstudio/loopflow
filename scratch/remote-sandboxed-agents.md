# 01: Sandboxed Agent Execution

## Problem

Today lfd runs agent commands as host subprocesses. That means a bad prompt or compromised tool can read local files, leak credentials, or interfere with other runs. We need strong isolation before remote deployment work (compose, EC2, remote Concerto) so agent execution has a bounded blast radius.

Who benefits:
- Loopflow users running autonomous waves on their dev machines
- Future hosted/remote users who need predictable isolation guarantees
- Maintainers, because containerized execution becomes the deployment primitive for later remote phases

Why now:
- This is Phase 01 in the remote roadmap and a prerequisite for Phase 02+.

## Approach

Implement a pluggable execution backend and ship Docker as the hardened backend.

### 1) Executor abstraction

Add an `AgentExecutor` trait used by lfd wave execution:
- `LocalProcessExecutor` (default, current behavior)
- `DockerExecutor` (new sandboxed behavior)

Selection comes from config/env:
- `executor.type = local|docker`
- `executor.image = loopflow/agent:<tag>`

### 2) Container lifecycle (chosen)

Run **one ephemeral container per agent step execution**:
- Create container with final agent command (`claude`, `codex`, `gemini`, `opencode`)
- Stream logs while running
- Capture exit code
- Stop/remove container immediately after completion or cancellation

This matches proven CI patterns (ephemeral job containers) and minimizes cross-step contamination.

### 3) Filesystem + repo model

Use a **per-repo Docker volume** as canonical workspace storage for docker execution:
- Main clone + sibling worktrees live in the volume
- Agent containers mount only that repo volume path
- No host home directory or arbitrary host bind mounts

lfd manages clones/worktrees through Docker-backed workspace operations so the host filesystem is not the execution surface.

### 4) Credential model (deny-by-default)

Pass only explicitly selected credentials per wave:
- Env vars (API keys, tokens)
- Read-only credential mounts for subscription auth (`~/.claude`, `~/.codex/auth.json`, etc.)
- Read-only git config mount

No wildcard host mounts. No implicit credential inheritance.

### 5) Container hardening baseline

For `DockerExecutor` containers:
- Non-root user
- `network_mode=bridge` (no host network)
- `privileged=false`
- `readonly_rootfs=true` + writable temp dir
- Capability drop (`cap_drop=["ALL"]`) unless explicitly required
- `auto_remove=false` while running (for logs/status), then explicit cleanup

### 6) Observability + control

- Stream stdout/stderr via Docker logs API into existing OutputHub/event pipeline
- Persist container id on running agent record for stop/cancel
- On cancel/failure: stop container, mark agent failed/cancelled, always attempt remove

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Keep host subprocess execution and add guardrails | Lowest engineering effort | Fails the core security goal; host remains the blast radius |
| Shell out to `docker run/stop/logs` instead of Docker API | Faster initial implementation | Harder error handling, weaker typed integration, brittle log streaming/parsing |
| MicroVM isolation (Firecracker/Kata) | Strongest isolation | Too heavy for Phase 01; slower to ship and raises local setup burden |

## Key decisions

- **Decision: use Docker API (`bollard`) instead of shelling out.** This gives typed lifecycle control and native async log streaming in Rust.
- **Decision: ephemeral container per step.** Better isolation and simpler recovery than keeping long-lived wave containers.
- **Decision: repo-in-volume for docker mode.** This enforces the phase promise that containers do not execute directly on host paths.
- **Decision: deny-by-default credentials.** Only wave-approved env vars/mounts are injected.
- **Decision: local executor remains default.** This follows the roadmap principle **"Sandbox first"** while avoiding regressions for users not ready for Docker.

Wild success we are designing for:
- Users flip `executor.type: docker` and runs behave the same functionally, but host risk drops dramatically.
- Multiple waves run concurrently without cross-wave interference.

Wild failure we are preventing:
- Over-broad mounts/credentials make containers "sandboxed in name only."
- Long-lived mutable containers create hidden state and flaky behavior across steps.

## Scope

- In scope:
  - `AgentExecutor` abstraction + `LocalProcessExecutor` + `DockerExecutor`
  - Docker agent image with Claude/Codex/Gemini/OpenCode headless CLIs
  - Per-repo Docker volume workspace model for docker execution
  - Credential injection policy (explicit env + read-only mounts)
  - Log streaming, cancellation, and cleanup via Docker API
  - Config switch (`executor.type`, `executor.image`) with local as default

- Out of scope:
  - Docker Compose stack for lfd + Postgres (Phase 02)
  - WAN auth and remote Concerto connectivity (Phases 03/05)
  - Editor remote file UX (Phase 06)
  - Domain-level egress allowlisting or full network policy engine

## Done when

- `cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test --all` passes.
- With `executor.type: docker`, running a wave creates agent containers and streams logs into lfd events.
- Cancelling a running wave stops and removes its active container.
- Agent containers can access only their repo volume + explicitly granted credentials.
- With `executor.type: local`, existing behavior still works without regressions.
