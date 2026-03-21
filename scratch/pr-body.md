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
