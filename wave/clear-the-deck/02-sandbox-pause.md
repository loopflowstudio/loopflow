# 02: Sandbox Pause and Daytona Evaluation

**Finish line:** Sandbox has one clear status: either a Daytona-backed spike proves a better path than Docker, or the custom sandbox path is demoted to an explicit experiment and hidden from the default deploy surface.

## Carried context

- Container mode already switches between `ExecutorType::Docker` and `ExecutorType::Sandbox` via `executor.sandbox`.
- `AdaptiveContainerExecutor` probes `docker sandbox` support at startup and falls back to Docker when the plugin is missing or fails.
- `SandboxExecutor` is still fully implemented against the Docker sandbox CLI, so keeping it means carrying real maintenance cost.

## What to build

1. Evaluate Daytona against the actual needs of a wave run: startup latency, git and worktree operations, context sync, harness compatibility, self-hosted reliability, and license fit.
2. If Daytona is good enough, spike the smallest executor integration that can run one end-to-end wave and document what would have to change next.
3. If Daytona is not good enough, simplify the current path: gate sandbox behind an explicit experimental flag, make Docker the only blessed backend, and delete dead branches in executor selection.
4. Update deploy docs and tests so the supported story is obvious.

## Uncertainty

- Daytona may solve container lifecycle but still miss worktree or context-sync needs.
- If the Docker sandbox CLI stabilizes quickly, a full Daytona migration may be unnecessary; the important outcome is a clear default, not novelty.

## Done when

- A written go or no-go verdict exists with concrete measurements.
- The default container execution path is unambiguous in code and docs.
- Unsupported sandbox combinations are no longer presented as normal user choices.
