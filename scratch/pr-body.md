## Try it!

```bash
cargo test -p loopflow operate
cargo test -p loopflow voice
cargo run -q -p loopflow --bin lf -- --help | rg -- '--operate'
```

Full gate checks run:

```bash
cargo fmt --all -- --check
cargo test -p loopflow
cargo build
cargo clippy --all-targets -- -D warnings
cargo test --all
```

Affected prompt goldens no longer include the old 152-line `<lf:rlm>` block at
the top. `lf --operate` injects the new loopflow operating guidance only when the
flag is set.

## Intent

Make loopflow's ambient prompt smaller and more explicit. Default prompts keep
surface instructions and selected context, while detailed loopflow operating
rules move behind `--operate` for wave-agent-style runs.

## Assumptions

- Voice guidance is repo/user taste, not a bundled default. `.lf/voice.md` and
  `~/.lf/voice.md` still work for interactive surfaces.
- `--operate` is a local launch flag, not an lfd wire field.
- `lf op land` consumes the scratch PR copy and clears scratch before landing.

## Key decisions

- Use `OPERATE.md` as the single source for opt-in operating guidance.
- Reuse `format_system_sections` for full prompts, Claude system prompts, and
  vendor skill seeds so system-safe sections do not drift.
- Keep lfd sessions at `operate: false` until a later wire-level design needs
  that control.
- Remove committed scratch artifacts because CI's `scratch-clear` job rejects
  them.

## Not included

- The later `loopflow.goal` handoff to read `OPERATE.md`.
- An `lf-prompt --operate` parity switch.
