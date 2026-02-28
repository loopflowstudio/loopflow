# MicroVM Agent Execution

## Vision

Replace Bollard-managed Docker containers with Docker Sandboxes microVMs for agent execution. Stronger kernel isolation, simpler executor lifecycle, same user-facing modes (native and container). Container mode internals change; everything above stays the same.

## Strategy

Lifecycle is `docker sandbox create` + `docker sandbox exec` + `docker sandbox rm`. Structurally closer to `LocalProcessExecutor` than `DockerExecutor` — shell out to CLI, capture PID + stdio, no Bollard.

Two-level capability gating decides executor per run:

1. **Platform capability (startup probe):** `docker sandbox version` → `create` → `exec -- true` → `rm`. Any failure disables sandbox path for process lifetime.
2. **Harness routing (per-run):** Claude and Gemini route to sandbox; everything else stays on `DockerExecutor`.

Runtime fallback: if sandbox launch/exec/cleanup fails for Claude/Gemini, retry once through `DockerExecutor` before marking the run failed.

`AdaptiveContainerExecutor` wraps both backends. `mode: container` resolves to `ExecutorType::Sandbox` (adaptive), keeping config simple.

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

- Sandbox launch latency: seconds from run start to agent process running (target: <5s, compare vs DockerExecutor)
- Sandbox cleanup success rate: % of runs where sandbox is fully removed after completion (target: 100%)
- Fallback trigger rate: % of sandbox failures that successfully fall back to DockerExecutor (target: 100%)
- Startup probe pass rate across platforms: macOS and Linux (target: 100% on supported Docker versions)
