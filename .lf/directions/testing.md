# Test without an installed Loopflow

Tests that construct session commands must supply their own `LF_BIN` fixture,
restore it afterward, and serialize environment changes with `test_env_lock`.
Reuse `TestLfBinGuard` in Task controller tests. Session spawning remains mocked.

For executable-resolution failures, reproduce with the compiled test binary:
unset `LF_BIN` and `CARGO_BIN_EXE_lf`, and use a PATH containing Git but no `lf`.
Verify the repair in that same environment. A pass under a developer's installed
Loopflow can hide the CI failure.
