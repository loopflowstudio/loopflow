# Review: Sandbox Executor Design + Wave Plan

## What was implemented

Wave plan for microVM agent execution (`wave/sandboxes/`) and a design doc for phase 1 (`scratch/sandboxes-executor-and-routing.md`).

- **Wave README:** Vision, strategy, phasing, goals, risks, metrics for replacing Bollard with Docker Sandboxes.
- **Wave items 01/02/03:** Phased delivery — executor+routing, integration+validation, full rollout+Bollard removal.
- **Design doc:** Detailed implementation plan for item 01 — `SandboxExecutor`, `AdaptiveContainerExecutor`, startup probe, config, recovery.

## Key choices

**`create` + `exec` + `rm` lifecycle.** Research into the Docker Sandbox CLI revealed `run` has no `-d` flag and host `docker exec` can't reach inside microVMs. The `create`/`exec` split gives control over credential injection, workspace setup timing, and clean stdout separation.

**Modeled after `LocalProcessExecutor`, not `DockerExecutor`.** `SandboxExecutor` shells out to CLI and captures PID + stdio — ~200 lines vs ~1600 for Bollard. Sandbox's built-in workspace sync eliminates volumes, tar sync, and shared clones.

**Adaptive routing over explicit config.** `AdaptiveContainerExecutor` probes at startup and routes transparently. Users don't need to know whether their machine supports sandboxes. Explicit `executor.type: docker` override is preserved.

**Env vars only for credentials.** No credential mounts. Matches `LocalProcessExecutor` pattern and sidesteps microVM filesystem visibility questions.

## How it fits together

`mode: container` resolves to `ExecutorType::Sandbox` (adaptive). At `lfd` startup, a probe creates a throwaway sandbox to verify the CLI works. If it does, Claude/Gemini runs go through `SandboxExecutor`; everything else through `DockerExecutor`. If a sandbox run fails at runtime, it falls back to Docker once before marking the run failed.

## Risks and bottlenecks

- **Sandbox CLI stability.** Young CLI surface. Breakage disables the sandbox path (mitigated by fallback).
- **`claude` template for Gemini.** Untested assumption that Gemini CLI works inside the `claude` template. Tracked explicitly in wave item 02's done-when criteria.
- **Per-run sandbox creation latency.** No pooling in phase 1. If `create` is slow, every Claude/Gemini run pays the cost.

## What's not included

- No code changes — this is the design + wave plan only.
- Codex/OpenCode routing (phase 2+).
- DinD support, stream reattach, Bollard removal (phase 2-3).
- Custom template strategy (phase 2+).

## Wave alignment

The design doc advances all four wave goals:
- Kernel isolation via microVM sandbox.
- Cleaner lifecycle: 3 CLI commands vs ~1600 lines of Bollard orchestration.
- Incremental adoption: Claude/Gemini first, Docker fallback always available.
- No user-visible behavior change: streaming, context files, cleanup all preserved.

Wave risks (CLI instability, DinD, Linux experimental, credential injection) are acknowledged and scoped appropriately — phase 1 defers DinD and Linux to validation, uses env vars for credentials with proxy as future work.
