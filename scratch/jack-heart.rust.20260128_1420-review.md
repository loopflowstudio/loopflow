# Review: Rust lfd trigger loops + scheduling

## What was implemented
- Added Rust `lfd` crate with gRPC server, HTTP health/status/metrics endpoints, SQLite store, scheduler, and background loops for loop/watch/cron/recovery.
- Extended wave model with `step_index` for flow position and persisted it through proto and SQLite.
- Added watch/cron activation logic, pending activation queueing, and recovery for stuck step runs.

## Key choices
- Separate polling loops per stimulus to match the daemon design doc and keep intervals independent.
- Store-first state: loops read/write SQLite state rather than keeping in-memory wave state.
- Concurrency enforced via a shared semaphore; watch/cron queue activations instead of directly ticking.
- Step runs are closed on tick completion/failure to avoid lingering active runs and cron misfires.

## How it fits together
`lfd` starts gRPC + HTTP servers and spawns four background loops. The loops update waves in SQLite, while the loop ticker bridges to `lf-core::tick_flow` via a store adapter. Wave state + step runs live in SQLite and drive health/status/metrics.

## Risks and bottlenecks
- `tick_flow` only supports linear steps; fork/choose/loop items fail and push waves toward error.
- Watch polling only inspects `origin/main`; repos with different default branches won't trigger.
- Cron polling reads all step runs each pass; could be slow at scale (Stage 5 expected to optimize).
- Missing session connect / event streaming; interactive flows are not yet usable end-to-end.

## What's not included
- Auth, remote access, or Postgres backend (later stages).
- PR polling / adaptive intervals.
- Session connect tracking or output streaming.
- FlowRun table (wave fields are reused for now).
