# lf wave gate review

## What was implemented

Added `lf wave <name>` as the foreground Wave progress runtime. It resolves a
wave, repeats bounded `lf -b goal <wave> --once` passes until Ctrl-C or
`wave/<wave>/STOP`, inheriting the terminal for the inner pass stream.

`lf loop` remains a command alias, but `lf wave` is the documented name. `lf goal`
now has a batch path that launches through the shared headless agent runner
instead of opening an interactive session.

## Key choices

- Kept the first slice intentionally narrow: progress arm only, no monitor,
  scheduler, or chat API.
- Used the existing headless agent launcher for `lf goal -b` so goal passes keep
  the same prompt assembly, harness config, and CLI availability checks as other
  batch runs.
- Treat spawn and wait failures as setup errors. Only a pass that actually ran
  produces a pass outcome.
- Reused the inner agent runner's existing durable logs instead of adding
  branch-local stream capture before the monitor exists to consume it.
- Removed branch-added local roadmap files during gate because current loopflow
  guidance makes Asana the roadmap source of truth.

## How it fits together

`lf wave` lives in `rust/loopflow/src/lf/commands/loop.rs` and shells out to the
current `lf` binary for each inner pass. The inner pass enters
`rust/loopflow/src/lf/commands/goal.rs`, which builds the wave goal prompt and,
when `-b` is active, calls `launch_agent` with streaming enabled. The outer wave
runtime inherits the terminal and lets the inner agent write its own durable
logs.

## Risks and bottlenecks

- A successful pass repeats immediately. That is the intended cadence, but a goal
  that exits quickly can run hot; failed passes have a short cooldown.
- Monitor work will need explicit stream capture later; this slice avoids
  writing logs nobody reads yet.
- `lf wave` is intentionally non-terminating. Automated smoke tests should use a
  STOP file or the unit-level `run_pass` coverage.
- Scratch files remain gate handoff artifacts. `lf op land` is expected to clear
  `scratch/` before merge.

## What's not included

- No monitor summarizer, standing chat summary, HTTP/WS chat API, or cron
  scheduler.
- No Asana roadmap mutation during gate. Follow-on roadmap changes should go
  through `lf op pm update`.
- No structured `<lf:pass-result>` parsing yet.

## Validation

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all
cargo test -p loopflow r#loop -- --nocapture
```

All passed locally on 2026-07-03.
