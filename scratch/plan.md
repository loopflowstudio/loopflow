# LOO-986 fix forward: a tmux window starts on the server's PATH

#1992 (`1a6c9d618a`) is merged. Verify found one acceptance line failing on it:
"A conversation opened into a pane by an agent does not get that agent's PATH".
Run notes: `scratch/verify.md`.

## Exists on this branch (`12a52f6aeb`)

- `rust/loopflow/src/lf/commands/open/tmux.rs` `open` resolves `tmux` on lf's
  `PATH` and runs the `new-window` client with `PATH` removed, so the window
  keeps the tmux server's.
- `work_open_tests::tmux_window_does_not_start_on_the_callers_path`: the
  stand-in `tmux` records its client `PATH`; the caller-only directory is absent.
  `cargo test -p loopflow --test integration work_open_tests::tmux`: 2 passed (implement).

## Remains

- Publish this branch as a second PR for LOO-986; CI runs the suites.
- Verify, on the merged fix, with a real tmux: `export PATH=/tmp/shim:$PATH`,
  `lf open --task <id> --host tmux --tab`, `ps -Eww` of the provider shows no
  `/tmp/shim`. The stand-in test shows only what lf hands the client.
- Verify, same probe, on Rex and cmux: their requests carry no environment, but
  the `rex` and `cmux` clients were not driven. Herdr's request sends `"env":{}`.
- Still not run from the first verify: Desktop opens; a plain open replayed
  after a moved conversation had ended.
