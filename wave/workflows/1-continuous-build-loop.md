---
asana_id: '1214270017822672'
---
# Continuous build loop

**Finish line:** A wave in `loops:` mode pulls items from its configured PM provider, spawns workers, ships PRs, and reports lifecycle back to PM. Runs overnight without human attention; conductor wakes up to shipped work.

## Context

Autonomous build means: wave config declares `loops` (or `crons`), lfd schedules workers, each worker calls `ingest` to pick an item (PM arbitrates no-double-pick), runs the wave's flow (build → deploy), and PR events feed back to PM for lifecycle updates.

What's needed:

- **Wave discovery and scheduling model** — lfd reads `wave/` on disk and reconciles with store. Wave config uses `loops` + `crons` + `triggers`, replacing the `mode` / `flow` / `workers` tangle
- **Concurrent ingest** — N workers in a pool call `ingest` simultaneously; the PM provider is the no-double-pick arbiter
- **CLI/daemon executor parity** — `CliExecutor` and `DaemonFlowExecutor` share `FlowEngine`. Regression tests pin parity across serialized, parallel, queued, cancelled, failed, and run-scoped-override cases

## Daily experience

Evening: you queue 5 items in Asana for a wave in loop mode. Morning: 3 PRs have landed, 1 is in review (CI-fix ran once), 1 item has a blocker comment back from the agent. Asana reflects each state. You review and move on.

## Done when

- Turning on loops for a wave spawns N workers that pull from PM without collisions
- PRs land without a manual step; PM items get lifecycle comments and completion
- Worker pool respects its budget, including across nested chord trees
- Regression test suite pins executor parity across the cases that matter
