## Try it!

```bash
cargo test -p loopflow operate
cargo test -p loopflow voice
cargo test -p loopflow reorder_args_operate_flag_after_step
cargo run -q -p loopflow --bin lf -- --help | rg -- '--operate'
cargo run -q -p loopflow --bin lf -- gate --operate --help | rg -- '--operate'
cargo run -q -p loopflow --bin lf-prompt -- --repo tests/fixtures/prompt/basic --surface headless --lfdocs false --diff-files false --diff false --clipboard false | rg -n '<lf:operate>|<lf:voice>|<lf:rlm>' || true
cargo run -q -p loopflow --bin lf-prompt -- --repo tests/fixtures/prompt/basic --surface headless --operate --lfdocs false --diff-files false --diff false --clipboard false | rg -n '<lf:operate>|lf op commit|</lf:operate>'
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
- Keep `lf-prompt --operate` available so parity fixtures can cover opt-in
  operate prompts without changing the default goldens.
- Keep lfd sessions at `operate: false` until a later wire-level design needs
  that control.
- Keep gate handoff files local-only because CI's `scratch-clear` job rejects
  committed `scratch/` contents.

## Not included

- The later `loopflow.goal` handoff to read `OPERATE.md`.
- A wire-level operate switch for lfd sessions.
