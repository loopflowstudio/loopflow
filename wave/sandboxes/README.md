# MicroVM Agent Execution

## Status (2026-02-28)

Exploratory only, off the production path. Container mode defaults to Docker; sandbox is opt-in (`executor.sandbox: true`) for validation and future rollout work.

## Vision

Replace Bollard-managed Docker containers with Docker Sandboxes microVMs for agent execution. Stronger kernel isolation, simpler executor lifecycle, same user-facing modes (native and container). Container mode internals change; everything above stays the same.

## Strategy

Lifecycle is `docker sandbox create` + `docker sandbox exec` + `docker sandbox rm`. Structurally closer to `LocalProcessExecutor` than `DockerExecutor` — shell out to CLI, capture PID + stdio, no Bollard.

Two-level capability gating decides executor per run:

1. **Platform capability (startup probe):** `docker sandbox version` → `create` → `exec` (direct or legacy separator syntax) → `rm`. Any failure disables sandbox path for process lifetime.
2. **Harness routing (per-run):** Claude routes to sandbox; everything else stays on `DockerExecutor`.

Runtime fallback: if sandbox launch/exec/cleanup fails for Claude, retry once through `DockerExecutor` before marking the run failed.

`AdaptiveContainerExecutor` wraps both backends. `mode: container` defaults to `ExecutorType::Docker`; set `executor.sandbox: true` to opt into sandbox routing.

### Validation approach

Hybrid: Rust unit tests for executor logic (command shape, cleanup, orphan recovery via fake docker scripts), shell scripts for real-environment validation across deployment surfaces. Probe-gate CI pattern — sandbox tests skip when plugin unavailable, activate automatically when it lands.

## Goals

- Kernel isolation boundary for agent runs
- Cleaner lifecycle than volume/tar-sync path
- Incremental adoption: Claude first, fallback always available
- No user-visible behavior change — streaming, context files, cleanup all work the same

## Risks

- **Docker Sandbox CLI drift.** Command surface changes across versions (`exec` syntax, `inspect` availability). Capability probes now gate on required commands and fail fast with explicit diagnostics.
- **Concerto DinD.** Bundled lfd container needs `docker sandbox` CLI plugin available. Current `loopflow/lfd` image has Docker CLI but no sandbox plugin (`docker sandbox` not recognized), so DinD validation is blocked until plugin distribution is solved.
- **Linux experimental.** Sandbox behavior on self-hosted Linux is experimental and unvalidated under load.
- **Credential injection.** Phase 1 uses env var injection. Credential proxy coverage for additional harnesses is an open question.

## Metrics

- Sandbox executor launches, runs, and cleans up Claude agents identically to DockerExecutor
- Fallback to Docker triggers correctly on sandbox failure
- Startup probe gates sandbox on/off without affecting other executor paths
- macOS validated (self-hosted + Concerto), Linux smoke-validated
