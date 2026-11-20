# LOO-986 fix forward: a tmux window starts on the server's PATH

#1992 (`1a6c9d618a`) is merged. Verify found one acceptance line failing on it:
"A conversation opened into a pane by an agent does not get that agent's PATH".
Run notes: `scratch/verify.md`.

## Exists on this branch (`12a52f6aeb`)

- `rust/loopflow/src/lf/commands/open/tmux.rs` `host_command` resolves `tmux`
  on lf's `PATH` for every client; `open` runs the `new-window` client with
  `PATH` removed, so the window keeps the tmux server's.
- `work_open_tests::tmux_window_does_not_start_on_the_callers_path`: the
  stand-in `tmux` records its client `PATH`; the caller-only directory is absent.
  `cargo test -p loopflow --test integration work_open_tests::tmux`: 2 passed (implement).

## Remains

- Publish this branch as a second PR for LOO-986; CI runs the suites.
- Verify on the merged fix: qa ran the real probe on this branch (`scratch/qa.html`):
  tmux, Rex and Herdr start the conversation without the caller's `PATH` entries.
  cmux is not installed here; not driven.
- Still not run from the first verify: Desktop opens; a plain open replayed
  after a moved conversation had ended.
