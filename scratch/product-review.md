# Review: explicit radio pub/sub

## What was implemented

`radio` now owns both operations on the ephemeral agent bus:

```text
lf radio pub [TEXT] [--channel NAME | --parent] [--from NAME]
lf radio sub [CHANNEL] [--json]
```

Bare `lf radio` prints subcommand help. The old `lf radio TEXT` and top-level
`lf sub` forms fail during argument parsing, including before the external-skill
fallback can reinterpret `sub`. Builtin guidance, webhook execution, docs,
examples, tests, and surface comments all use the explicit grammar.

The product wave also gains its authored daily cadence: the standard `wave`
flow is scheduled at 08:00 UTC, and the wave-chat KR now names `lf radio sub` as
the bus-only subscription surface.

## Key choices

- Kept the store bus and its publish/subscribe implementations unchanged. This
  is command ownership, not a transport rewrite.
- Used a nested Clap subcommand enum so help exposes the bus vocabulary in one
  place and bare `radio` cannot accidentally publish stdin.
- Reserved the retired top-level `sub` spelling as an always-failing hidden
  parser branch. Without it, Loopflow's external-subcommand fallback would try
  to execute a skill named `sub`, which would make the removal ambiguous.
- Migrated webhook argv and builtin prompts atomically. Runtime-generated agent
  instructions cannot teach a CLI spelling the same build no longer accepts.
- Reused the standard `wave` flow for the product heartbeat instead of adding a
  product-specific scheduler concept.

## How it fits together

Clap parses `radio pub` into the existing store INSERT path and `radio sub` into
the existing forward-poll path. The listener, cursor semantics, channel-prefix
matching, and byline rules remain exactly where they were; only CLI dispatch and
the text that invokes it changed. The product resident re-reads `GOAL.md` and
dispatches the existing wave flow when its cron becomes due.

## Risks and bottlenecks

- This is an intentional breaking CLI change. Any caller outside this repository
  still using `lf radio TEXT` or `lf sub` will fail with parser guidance.
- Cron schedules are evaluated against UTC by the resident. The product cadence
  is therefore 08:00 UTC, regardless of the host's local timezone or daylight
  saving time.
- Agent-bus subscription remains a polling loop. This change does not alter its
  polling cadence, at-most-live observation semantics, or one-hour sweep window.

## What's not included

- No compatibility aliases for the retired command forms.
- No bus schema, delivery, cursor, retention, or attribution changes.
- No timezone field or local-time cron interpretation.
- No new chat behavior; `lf chat` remains the sole human thread surface.

## Validation

- `uv run python scripts/test.py --all` — all six suites passed: Python, Rust
  format/clippy/tests, website, Swift package/boundary checks, CLI e2e smoke,
  and signed macOS `build-for-testing`.
- CLI smoke: `lf radio --help` exposes only `pub` and `sub`; bare `lf radio`
  prints the same help; `lf sub goals` fails with `use lf radio sub` guidance.
- Stale-command scan over user docs, builtins, runtime docs, scripts, Swift, and
  tests found no executable `lf radio TEXT` or top-level `lf sub` examples.
- `git diff --check` passed.

