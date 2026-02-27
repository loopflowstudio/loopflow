# MicroVM Agent Execution

## Vision

Replace Bollard-managed Docker containers with Docker Sandboxes microVMs for agent execution. Stronger kernel isolation, simpler executor lifecycle, same user-facing modes (native and container). Container mode internals change; everything above stays the same.

## Strategy

Docker Sandboxes doesn't expose `create/exec/stop` — only `run`, `ls`, `inspect`, `rm`, `version`. So the execution model is `docker sandbox run -d` for lifecycle + `docker exec` for command execution. This gives us PID + stdio piping matching `LocalProcessExecutor` semantics.

Two-level capability gating decides executor per run:

1. **Platform capability (startup probe):** `docker sandbox version` → `run -d` → `docker exec -- true` → `rm`. If any step fails, disable sandbox path entirely.
2. **Harness routing (per-run):** Claude and Gemini route to sandbox; everything else stays on `DockerExecutor`.

Runtime fallback: if sandbox launch/exec/cleanup fails for Claude/Gemini, retry once through `DockerExecutor` before marking the run failed.

`AdaptiveContainerExecutor` wraps both backends. `mode: container` resolves to `ExecutorType::Sandbox` (adaptive), keeping config simple.

### Key bet

Docker Sandboxes is usable today if treated as **lifecycle API + workspace sync**, with `docker exec` for generic command execution.

### Phasing

Phase 1: Claude + Gemini only, with full `DockerExecutor` fallback. No DinD guarantee, no stream reattach on restart.

Phase 2+: Codex/OpenCode rollout, DinD support contract, restart rehydration, Bollard removal.

## Goals

- Kernel isolation boundary for agent runs
- Cleaner lifecycle than volume/tar-sync path
- Incremental adoption: Claude/Gemini first, fallback always available
- No user-visible behavior change — streaming, context files, cleanup all work the same

## Risks

- **Docker Sandbox CLI instability.** The CLI surface is young. Breakage on updates could disable the sandbox path entirely (mitigated by fallback to DockerExecutor).
- **Concerto DinD.** Bundled lfd container needs `docker sandbox` CLI plugin available. Unvalidated.
- **Linux experimental.** Sandbox behavior on self-hosted Linux is experimental and unvalidated under load.
- **Credential injection.** Phase 1 uses env var injection. Credential proxy coverage for Claude/Gemini is an open question.

## Metrics

- Sandbox executor launches, runs, and cleans up Claude agents identically to DockerExecutor
- Gemini path passes automated smoke test
- Fallback to Docker triggers correctly on sandbox failure
- Startup probe gates sandbox on/off without affecting other executor paths
- macOS validated (self-hosted + Concerto), Linux smoke-validated
