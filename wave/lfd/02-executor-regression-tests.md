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

## Known constraints

- Concerto macOS app tests (`xcodebuild test`) fail to bootstrap locally before test execution starts. With `CODE_SIGNING_ALLOWED=NO`, dyld rejects `Concerto.debug.dylib` under system policy; with ad-hoc signing, the app is killed before XCTest connects. Swift package tests pass. This may or may not reproduce in CI — needs verification.
- Targeted terminal session attach tests already exist (added on the connection-info branch). This item is the broader suite beyond that path.

## Approach

Integration tests that exercise `DaemonFlowExecutor` with real `lf` subprocess spawning, not mocks. The test harness should set up a minimal wave config, trigger execution, and assert on the resulting run/session state.
