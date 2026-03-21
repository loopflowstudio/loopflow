## Try it!

```bash
cargo fmt --check
cargo clippy -p loopflow -- -D warnings
cargo test -p loopflow
```

What you should see:
- CLI flows and daemon-supervised wave runs both execute through the shared `FlowEngine`.
- Daemon-launched `lf` children inherit `LFD_WAVE_ID`, `LFD_RUN_ID`, and `LF_RUN_ID`; interactive sessions also get `LFD_SESSION_ID`.
- Nested interactive xor/loop waits resume from the stored execution cursor instead of restarting the top-level item.
- Stopping a waiting wave cancels its active terminal session instead of leaving orphaned interactive state behind.

## Intent

Finish the real CLI executor refactor so `lfd` stops reimplementing flow semantics itself. The daemon now supervises per-step `lf` processes and interactive terminal sessions while the shared execution engine owns sequencing, xor routing, loop traversal, and fork/and behavior for both CLI and daemon entrypoints.

## Assumptions

- `tmux` is available for the preferred daemon-hosted interactive-step path; when it is not, the older wrapped terminal-session path remains the fallback.
- Journal polling once per second is still acceptable for progress visibility.
- `or` flow items remain intentionally unsupported and should still surface a not-implemented error.

## Key decisions

- Persist a serialized nested execution cursor on `WaveRun` so waits inside xor/loop structures resume exactly where they paused.
- Resolve the current interactive step from that cursor instead of top-level `step_index` alone.
- Keep journal events as observability, not control flow: the daemon advances from the shared engine cursor and only uses journal replay for client-visible progress.
- Cancel active terminal sessions when a waiting wave is stopped so cancellation semantics match headless step cancellation.
- Retry branch renames after worktree moves when git's branch metadata briefly lags, which removes a flaky wave-worktree edge case from the validation path.

## Not included

- `or` (multi-select) flow execution.
- Push-based journal ingestion.
- A fix for the local Concerto UI-test bootstrap crash.
