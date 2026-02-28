# Review: Sandbox Executor and Adaptive Routing

## What was implemented

Full implementation of wave item 01 — `SandboxExecutor`, `AdaptiveContainerExecutor`, startup probe, config changes, and recovery. Plus the wave plan (`wave/sandboxes/`) and design doc (`scratch/sandboxes-executor-and-routing.md`).

### New files

- **`executor/sandbox.rs`** (~310 lines with tests): `SandboxExecutor` implementing `AgentExecutor` trait. Shells out to `docker sandbox create/exec/rm` via `tokio::process::Command`. Credentials injected via `-e` flags on exec. Recovery lists managed sandboxes via `docker sandbox ls` + `inspect`, removes orphans matching `lf-*` prefix.

- **`executor/adaptive.rs`** (~480 lines with tests): `AdaptiveContainerExecutor` wrapping `SandboxExecutor` + `DockerExecutor`. Routes Claude/Gemini to sandbox when available, everything else to Docker. Background probe at startup stores result in `Arc<OnceLock<bool>>`. Runtime fallback to Docker on sandbox failure.

### Modified files

- **`config.rs`**: Added `ExecutorType::Sandbox` variant with `#[non_exhaustive]`. Added `executor.sandbox` bool (default `true`) to `RawExecutorConfig`. `ModeProfile::for_mode()` resolves `mode: container` to `Sandbox` (adaptive) or `Docker` based on the flag.

- **`executor/mod.rs`**: Added `pub(crate) mod adaptive;` and `pub(crate) mod sandbox;`.

- **`executor/wave/mod.rs`**: Added `ExecutorType::Sandbox` arm — constructs `AdaptiveContainerExecutor`.

- **`service/compose.rs`**: Compose file validation accepts `ExecutorType::Sandbox` alongside `Docker`.

### Documentation

- **Wave plan**: README + 3 phased items (executor+routing → integration+validation → full rollout).
- **Design doc**: Updated to reflect implementation decisions (async probe, `OnceLock`, `executor.sandbox` opt-out).

## Key choices

**`create` + `exec` + `rm` lifecycle.** `docker sandbox run` has no `-d` flag; host `docker exec` can't reach inside microVMs. The split gives control over credential injection, workspace setup timing, and clean stdout separation.

**Modeled after `LocalProcessExecutor`.** SandboxExecutor spawns `docker sandbox exec` as a subprocess, captures PID + stdio, enforces timeout, kills on terminate. ~200 lines of production code vs ~1600 for Bollard. Sandbox's built-in workspace sync eliminates volumes, tar sync, and shared clones.

**Non-blocking probe.** `Arc<OnceLock<bool>>` set by a background task. Before probe completes, Claude/Gemini fall through to Docker (same as probe failure). Logs elapsed time and failure step.

**`executor.sandbox: false` opt-out.** Keeps the "mode manages executor.type" invariant intact. Explicit `executor.type` in YAML is still rejected. The opt-out flag gives operators a narrow escape hatch without breaking the config model.

**Env vars only for credentials.** No credential mounts. Same approach as `LocalProcessExecutor`. Sidesteps questions about home directory visibility inside microVMs.

## How it fits together

```
lfd.yaml: mode: container
    → ModeProfile: ExecutorType::Sandbox (if executor.sandbox != false)
    → WaveExecutor builds AdaptiveContainerExecutor
    → Background probe: docker sandbox version → create → exec → rm
    → Per run:
        if probe passed && (claude || gemini):
            SandboxExecutor.run()  → fallback to DockerExecutor on error
        else:
            DockerExecutor.run()
```

Cleanup delegation: `cleanup_ephemeral_worktree` and `cleanup_wave_workspace` always go through `DockerExecutor`, which gracefully handles missing Docker volumes (early-return when volume doesn't exist). Recovery merges reports from both backends.

## Risks and bottlenecks

- **Sandbox CLI stability.** Young CLI surface. Breakage disables the sandbox path (mitigated by fallback + probe).
- **CLI command assumptions.** `scratch/questions.md` notes that `docker sandbox create/exec/stop` may not exist on current CLI versions. The probe catches this at startup.
- **Per-run sandbox creation latency.** No pooling in phase 1. If `create` is slow, every Claude/Gemini run pays the cost.
- **`claude` template for Gemini.** Untested assumption. Tracked in wave item 02.

## What's not included

- Codex/OpenCode sandbox routing (phase 2+).
- DinD support, stream reattach, Bollard removal (phase 2-3).
- Custom template strategy (phase 2+).
- Credential proxy (phase 2+).

## Test coverage

| Area | Tests |
|------|-------|
| SandboxExecutor | run + stream output, timeout + cleanup, recovery (managed vs unmanaged sandboxes) |
| AdaptiveContainerExecutor | route claude → sandbox, fallback on failure, route codex → docker, probe pending → docker, terminate dispatch, recovery merge |
| Config | container → Sandbox, sandbox disabled → Docker, explicit type rejected |
| Compose | accepts Sandbox executor type |
