## Try it!

```bash
cargo run -q -p loopflow --bin lf -- radio --help
cargo run -q -p loopflow --bin lf -- radio sub product
```

In another terminal, publish a frame:

```bash
cargo run -q -p loopflow --bin lf -- radio pub \
  -c product --from demo "button audit finished"
```

The subscriber prints `[product] demo: button audit finished`. Bare `lf radio`
shows the `pub`/`sub` help, while `lf radio "text"` and `lf sub product` fail at
parsing.

Run the complete repository gate:

```bash
uv run python scripts/test.py --all
```

All six suites pass on this branch: Python, Rust format/clippy/tests, website,
Swift package/boundary checks, CLI e2e smoke, and the signed macOS test build.

## Intent

Make the agent bus read as one explicit namespace. Publishing and subscribing
are now `lf radio pub` and `lf radio sub`; `lf chat` remains the separate human,
durable thread. The change updates every repository-owned caller and prompt in
the same build, and restores the product wave's authored daily operating cadence.

## Assumptions

- Removing the old forms is intentional; internal CLI compatibility is not
  preserved unless a migration is requested.
- The store bus remains the right transport for both operations: publishing is
  an INSERT and subscribing is a forward poll, with no listener in the path.
- Resident cron expressions use UTC. `0 0 8 * * * *` schedules the product wave
  flow at 08:00 UTC.

## Key decisions

- Model `pub` and `sub` as nested Clap subcommands, so bare `radio` is help and
  can never consume stdin as an accidental publish.
- Keep a hidden, always-failing parser reservation for top-level `sub`; this
  prevents the external-skill fallback from treating the removed command as a
  skill name.
- Leave bus storage, delivery, cursor, retention, and byline behavior untouched.
- Schedule the existing standard `wave` flow instead of adding a product-only
  flow or scheduler path.

## Not included

- Compatibility aliases for `lf radio TEXT` or `lf sub`.
- Bus transport or delivery-semantics changes.
- Local-time or timezone-aware cron scheduling.
- Changes to `lf chat` or the human thread.

