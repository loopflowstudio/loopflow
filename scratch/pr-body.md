## Try it!

```bash
cargo build -p loopflow
touch wave/goals/STOP
target/debug/lf wave goals
rm wave/goals/STOP
```

You should see `lf wave` resolve the `goals` wave and exit cleanly because the
STOP file is present. To run the real progress loop, omit the STOP file:

```bash
target/debug/lf wave goals
```

Each pass runs `lf -b goal goals --once` and writes a stream log under
`wave/goals/streams/`.

Validation run:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all
```

## Intent

Move Wave progress from model-owned looping into a loopflow-owned foreground
runtime. `lf wave <name>` now owns the outer loop and repeatedly launches bounded
goal passes, while preserving `lf goal` as the inner human/agent unit.

## Assumptions

- The first shippable slice is the progress arm only.
- Stream logs are runtime artifacts and should stay ignored.
- Asana remains the roadmap source of truth; this branch does not add local
  numbered roadmap files.

## Key decisions

- `lf wave` is the public command name; `lf loop` stays as an alias.
- `lf goal -b` uses the shared headless agent launcher rather than a new custom
  path.
- Setup failures bubble as errors, while failed inner passes cool down and keep
  the outer loop alive.

## Not included

- Monitor summarization, chat API, cron scheduling, and structured pass-result
  parsing.
- Asana roadmap updates for the follow-on Wave runtime work.
