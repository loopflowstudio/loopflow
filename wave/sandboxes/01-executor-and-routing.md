# 01: Sandbox Executor and Adaptive Routing

Core implementation: `SandboxExecutor`, `AdaptiveContainerExecutor`, capability gating, and config.

## What we're trying to learn

Does `docker sandbox run -d` + `docker exec` reliably replace the Bollard lifecycle for Claude and Gemini runs? The probe and fallback exist because we're not certain yet.

## Scope

### SandboxExecutor

Shells out to Docker CLI only:

- `docker sandbox run -d --name <sandbox_id> -w <workspace> claude`
- `docker exec <sandbox_id> -- <agent command ...>`
- `docker sandbox rm <sandbox_id>`

Keeps an active map (`agent_id -> sandbox_id`) for termination dispatch. Credentials injected via `--env KEY=VALUE` on `docker exec`.

### AdaptiveContainerExecutor

Wrapper owning:

- `sandbox: SandboxExecutor`
- `docker: DockerExecutor`
- cached startup probe result

`run()` checks probe result, then routes by harness (`cmd[0]`): Claude/Gemini to sandbox, everything else to Docker. `terminate()` dispatches to whichever backend owns the active agent.

### Capability gating

Startup probe (run once, cache result):

1. `docker sandbox version`
2. `docker sandbox run -d --name <probe> claude`
3. `docker exec <probe> -- true`
4. `docker sandbox rm <probe>`

Any failure disables sandbox path for the process lifetime.

### Safety fallback

If sandbox run fails at runtime (launch/exec/cleanup error), log reason and retry once through `DockerExecutor`.

### Config

Add `ExecutorType::Sandbox` variant. `mode: container` resolves to `ExecutorType::Sandbox` (adaptive behavior). No user-visible config split for harness routing.

```rust
pub enum ExecutorType {
    Local,
    Docker,   // legacy fallback
    Sandbox,  // adaptive: sandbox where supported, docker fallback otherwise
}
```

## Done when

- `SandboxExecutor` implemented with `run -d` + `docker exec` + `rm`
- `AdaptiveContainerExecutor` routes Claude/Gemini to sandbox, others to Docker
- Startup probe gates sandbox path on/off
- Runtime fallback to Docker on sandbox failures
- Container mode defaults to `ExecutorType::Sandbox` adaptive path
- stdout/stderr streaming unchanged through `docker exec` piping
