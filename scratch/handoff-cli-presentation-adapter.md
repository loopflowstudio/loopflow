# CLI Presentation Adapter for Interactive Handoffs

**Done When:** `lf handoff present <id>` attaches and execs into the terminal
session so a human at a CLI can participate in the interactive work.

## What

The Mac app has `HandoffAttachSheet` — calls `lf handoff attach --json`, gets
the `InteractiveHandoffAttach` descriptor, and renders Ghostty. The CLI had no
equivalent: `attach` returns the descriptor as JSON, but never connects the human
to the terminal.

`lf handoff present <session_id>` closes this gap:

1. Records first-attach evidence via the store (`attach_interactive_handoff`).
2. Reads the descriptor (argv, cwd, environment).
3. Replaces the `lf` process with `exec(argv)` — the terminal session inherits
   stdin/stdout/stderr.
4. When the terminal exits, control returns to the shell. The human can then
   `lf handoff complete/back/fail`.

No `--json` flag — the process is replaced; there is no stdout to serialize.

## Files Changed

- `rust/loopflow/src/lf/mod.rs` — `HandoffCommand::Present` variant + parse test
- `rust/loopflow/src/lf/commands/handoff.rs` — `present()` exec adapter + handler
- `rust/loopflow/tests/handoff_tests.rs` — integration test (`true` as argv)
- `README.md` — updated handoff lifecycle example
- `docs/lf.md` — documented `present` in the CLI reference
