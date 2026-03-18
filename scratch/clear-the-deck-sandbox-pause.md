# 02: Sandbox Pause and Daytona Evaluation

**Finish line:** Sandbox has one clear status: either a replacement proves a better container path than Docker for real wave runs, or the custom sandbox path is demoted to an explicit experiment and removed from the default deploy surface.

## Carried context

- The deployment/auth collapse has landed: user-facing docs and compose defaults now teach `native` / `container` deployment and `local` / `studio` auth, with `mode: container` as the persistent entrypoint.
- Docker is already the blessed container executor in docs and compose. `executor.sandbox` remains as the last experimental backend override in the configuration reference.
- Container mode still derives `ExecutorType::Docker` vs `ExecutorType::Sandbox` from `executor.sandbox`, and `WaveExecutor` still builds `AdaptiveContainerExecutor` when sandbox is enabled.
- `AdaptiveContainerExecutor` probes `docker sandbox` support at startup, routes sandbox-eligible harnesses through `SandboxExecutor`, and falls back to `DockerExecutor` when the probe or run fails.
- Compose rendering and config tests still treat sandbox as a supported container executor type, so any demotion or deletion has to update code, docs, and tests together.

## What to build

1. Get the verdict first: measure Docker sandbox against the actual needs of a wave run, and compare that with any replacement candidate such as Daytona on startup latency, worktree operations, context and credential sync, harness compatibility, cleanup, and self-hosted reliability.
2. If no replacement clearly wins, simplify the current path: make Docker the only supported default, reduce `executor.sandbox` to an explicit experiment or delete it, and remove dead branches from executor selection and compose generation.
3. If a replacement clearly wins, land the smallest end-to-end spike that proves loopflow can start, execute, recover, and clean up a real wave under it without re-expanding the deployment matrix.
4. Update docs and tests together so the configuration reference, compose output, and executor validation all describe the same status.

## Uncertainty

- Daytona may solve lifecycle management but still miss loopflow's worktree or credential assumptions.
- Keeping an experimental sandbox flag may still be too much surface area if compose, docs, and tests must support it everywhere.
- Some internal harness flows may rely on the current adaptive fallback behavior; removing it could expose gaps in Docker-only execution.

## Done when

- A written go / no-go verdict exists with concrete measurements.
- Code has one default container executor path and at most one clearly experimental alternative.
- Docs, compose output, and tests no longer imply that sandbox and Docker are equally supported.
