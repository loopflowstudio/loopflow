# Testing Loopflow boundaries

When changing Wave chat operations, include the parent module's HTTP and SSE
tests. Selecting only `runner::tests` or steering-named tests misses them.

```bash
cargo nextest run -p loopflow --lib -E 'test(controller::wave::)' --no-fail-fast
```

Retired operations must be rejected without journaling, while ordinary messages
and bare interrupts retain their behavior.

When changing Task controls, include the GitHub-cache integration tests as well
as controller tests. Bare interrupts prove local control during GitHub outages;
steering publishes to Linear and belongs with the mocked Linear boundary tests.

```bash
cargo nextest run -p loopflow --test task_github_cache_tests --no-fail-fast
```

When changing Linear response shapes, run the client tests and PM-operation
consumers together. Team migration also reads issue comments; its fixtures must
include the requested pagination metadata.

```bash
cargo nextest run -p loopflow --lib -E 'test(pm::linear::) | test(ops::pm::) | test(ops::linear_observe::)' --no-fail-fast
```

## Test without an installed Loopflow

Tests that construct session commands must supply their own `LF_BIN` fixture,
restore it afterward, and serialize environment changes with `test_env_lock`.
Reuse `TestLfBinGuard` in Task controller tests. Session spawning remains mocked.

For executable-resolution failures, reproduce with the compiled test binary:
unset `LF_BIN` and `CARGO_BIN_EXE_lf`, and use a PATH containing Git but no `lf`.
Verify the repair in that same environment. A pass under a developer's installed
Loopflow can hide the CI failure.
