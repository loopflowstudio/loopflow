# CI-fix: tmux bypass in tests

## Root cause

`WaveExecutor::execute()` checks `tmux_available()` before deciding whether
to use the injected runner (MockRunner in tests) or launch a real tmux session.
Ubuntu CI runners have tmux pre-installed, so ~33% of CI runs hit the tmux
path, bypassing MockRunner entirely and getting exit_code=1 from a failed
real session launch.

## Why gate didn't catch it

Gate runs locally on macOS where tmux is also available — but locally the
tmux session launch likely succeeds (or the test environment differs enough
that it works). The flakiness was specific to CI's Ubuntu environment where
tmux exists but `lf` is not installed, causing the launched session to fail.

## Adaptation

Tests that inject a mock runner via `with_runner()` should not have their
mock bypassed by environment detection. The fix adds `disable_tmux: bool` to
`WaveExecutor`, set `true` in the test-only constructor.

**Pattern to watch for**: any `execute()` path that checks for external tools
before using the injected mock. The check should respect test construction.
