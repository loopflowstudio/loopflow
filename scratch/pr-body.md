## Try it!

```bash
cargo fmt --check
cargo clippy -p loopflow -- -D warnings
cargo test -p loopflow
```

What you should see:
- The daemon and CLI both drive flow execution through the shared `FlowEngine`.
- Daemon-launched `lf` children inherit `LFD_WAVE_ID`, `LFD_RUN_ID`, and `LF_RUN_ID`; interactive sessions also get `LFD_SESSION_ID`.
- Interactive wave steps launch into daemon-hosted tmux sessions when tmux is available, and waiting runs resume from nested xor/loop positions instead of restarting the top-level item.

## Intent

Finish the real CLI executor refactor. The daemon now stops reimplementing flow semantics itself and instead supervises per-step `lf` processes while the shared `FlowEngine` handles sequencing, xor routing, loops, and fork/and.

## Key decisions

- Persist a serialized execution cursor on `WaveRun` so nested waits resume exactly where they paused.
- Resolve interactive attach requests from the current execution cursor instead of top-level `step_index` alone.
- Prefer daemon-hosted tmux sessions for interactive steps, with the old wrapped command path kept only as a fallback when tmux is unavailable.
- Update `flow_parents` from the exact step/op/fork being executed so nested resumes stay accurate.

## Not included

- `or` (multi-select) flow execution; the shared engine still returns a deliberate not-implemented error there.
- A fix for the local Concerto UI-test bootstrap crash.
