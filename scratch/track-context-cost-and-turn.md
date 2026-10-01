# LOO-348: context cost and turn latency by week

Draft, 2026-10-01. Jack Heart requested a Wave instrument for the LOO-298
speed baselines so LOO-346's budgets and later prompt changes are judged by
numbers.

## Shape

`lf usage --weekly` reads Session history from the Home's store and reports
seven-day weeks anchored on 2026-09-30T00:00Z, starting with the baseline week
that ends there. Each week carries:

- input:output ratio and input per step, by step kind (skill), harness and Task
- turn minutes (median, p90) by harness and peak-input band
  (<100k, 100–150k, 150–200k, 200k+)
- silent time: gaps over 10 minutes between retained events inside a turn that
  later completed

An unfiltered run publishes the latest complete week to four Intelligence Wave
metrics through the existing producer path (`publish_metric_observations`),
instrument `context-cost`. `telemetry-daily` gains the step, so the daily cron
keeps the readings fresh. No new store, table, DTO mirror or script.

Contracts: `wave/intelligence/metrics/context-{input-output-ratio,
input-per-step,slow-turn-minutes,silent-hours}.md`. Baselines and the KR/target
plan live in `performance/context-cost.md`.

## Delete — do not maintain

Nothing. This is a new reader over existing evidence.

## Remaining

- Apply the chapter plan (KRs and metric targets) after the contracts reach
  main: `validate_chapter_targets` reads contracts from the Wave's repository,
  so targets cannot resolve before merge. Command is in
  `performance/context-cost.md`.
- First real reading waits on an installed lf carrying this branch: the
  installed 0.12.28 store predates `session_events`, so the report could not
  be run against Jack's Home from this worktree.

## Check

`cargo test -p loopflow --lib -- context_cost event_times_follow`: 5 passed; `cargo clippy --all-targets -- -D warnings` clean. Affected suites: gate.
