## Try it!

```bash
cargo run -p loopflow --bin lf -- op reset-waves --help
uv run python scripts/test.py
```

For a full local gate:

```bash
cargo fmt --check
cargo clippy -- -D warnings
uv run python scripts/test.py --all
```

`reset-waves` is destructive by design: it kills every `lf-*` tmux session on the
machine. Use `--help` for a safe smoke check, or run the command only when you
want a real wave fresh start.

## Intent

Make wave launch recovery boring after crashes. Concerto no longer treats a dead
tmux session name as a permanent Start-button blocker, and `lf op reset-waves`
gives operators a bulk fresh-start command for stale `lf-*` sessions and endpoint
files.

## Assumptions

- A probed live wave endpoint is the source of truth for "already running."
- A tmux session with no live endpoint is stale enough to reclaim after a short
  grace probe.
- lfd owns registry reconciliation after sessions are killed.

## Key decisions

- Concerto blocks only on a live endpoint, not on a raw `.wave-endpoint` file or
  a tmux session name.
- Existing tmux sessions get three endpoint probes before reclaim, so a wave
  still booting has time to publish.
- `lf op reset-waves` is rejected as a flow op item because it has broad machine
  side effects.

## Not included

- No lfd registry write path in the reset command.
- No README update; this is an operator command and launcher behavior fix covered
  by CLI help and tests.

## Validation

- `cargo fmt --check`
- `cargo clippy -- -D warnings`
- `uv run python scripts/test.py` passed.
- `uv run python scripts/test.py --all` passed Python, Rust, website, Swift, and
  e2e. Concerto UI failed locally before UI-test bootstrap:
  `ConcertoUITests-Runner ... Test crashed with signal kill before establishing
  connection.`
