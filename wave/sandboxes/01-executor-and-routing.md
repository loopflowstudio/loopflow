# 01: Sandbox Executor and Adaptive Routing

**Finish line:** `SandboxExecutor` runs Claude agents via `docker sandbox create/exec/rm`, `AdaptiveContainerExecutor` routes by harness with startup probe, and `cargo test --all` passes.

## What we're trying to learn

Does `docker sandbox create` + `docker sandbox exec` + `docker sandbox rm` reliably replace the Bollard lifecycle for Claude and Gemini runs? The probe and fallback exist because we're not certain yet.

## Scope

### SandboxExecutor

Shells out to Docker Sandbox CLI via `tokio::process::Command`. Modeled after `LocalProcessExecutor` — spawn subprocess, capture PID + stdio, no Bollard.

- `docker sandbox create --name lf-{agent_id} claude {cwd}`
- `docker sandbox exec -e KEY=VALUE ... -w {cwd} lf-{agent_id} -- {cmd...}`
- `docker sandbox rm lf-{agent_id}`

Credentials injected via `-e` flags on exec (env vars only, no mounts). Keeps an active map (`agent_id -> SandboxState`) for termination dispatch.

### AdaptiveContainerExecutor

Wraps `SandboxExecutor` + `DockerExecutor`. Routes by startup probe result and harness (`cmd[0]`): Claude/Gemini to sandbox, everything else to Docker. Runtime fallback to Docker on sandbox failure.

### Startup probe

Runs once at `lfd` startup, caches result for process lifetime:

1. `docker sandbox version`
2. `docker sandbox create --name lf-probe claude /tmp`
3. `docker sandbox exec lf-probe -- true`
4. `docker sandbox rm lf-probe`

Any failure disables sandbox path for the process.

### Config

`ExecutorType::Sandbox` variant. `mode: container` resolves to `Sandbox` (adaptive). Explicit `executor.type: docker` bypasses sandbox entirely.

### Recovery

Startup orphan cleanup: `docker sandbox ls` → match `lf-*` → `rm` + fail orphaned DB runs.

## Done when

- `SandboxExecutor` implements `AgentExecutor` via `docker sandbox create/exec/rm`
- `AdaptiveContainerExecutor` routes Claude/Gemini to sandbox, others to Docker
- Startup probe gates sandbox on/off, logs result
- Runtime fallback to Docker on sandbox failure with logged reason
- `mode: container` resolves to `ExecutorType::Sandbox`
- Orphan cleanup on startup
- `cargo test --all` and `cargo clippy -- -D warnings` pass
