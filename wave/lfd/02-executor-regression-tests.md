# 02: Executor Regression Tests

**Finish line:** A test suite pins parity between `CliExecutor` and `DaemonFlowExecutor` for the cases that matter: serialized vs parallel waves, queued activations, CI-fix runs, cancellation, failure propagation, and run-scoped overrides.

## Context

`FlowEngine` is shared between CLI and daemon, but step execution differs (in-process vs process-supervised). The design calls for tests that prove parity across execution modes. None exist yet.

The terminal session attach tests added on the connection-info branch are focused and targeted. This item is the broader regression suite that exercises the executor lifecycle paths end to end.

## What to cover

- Serialized wave execution (steps run in sequence)
- Parallel wave execution (fork/and)
- Queued activations (multiple triggers while a run is in progress)
- CI-fix flow (trigger on CI failure, spawn repair run)
- Cancellation propagation (cancel a run, verify child processes terminate)
- Failure propagation (step failure → run failure → wave state update)
- Run-scoped overrides (direction, area overrides per-run)

## Approach

Integration tests that exercise `DaemonFlowExecutor` with real `lf` subprocess spawning, not mocks. The test harness should set up a minimal wave config, trigger execution, and assert on the resulting run/session state.
