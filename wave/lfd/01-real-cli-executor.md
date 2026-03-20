---
linear_id: b854c1c9-b49f-47f6-be6f-381f7c7cb1b0
---
# 01: Real CLI Executor

**Finish line:** when `lfd` decides a run should start, it supervises a normal `lf <flow-or-step>` process in the correct worktree and environment instead of executing flows through a second bespoke daemon executor.

## Context

The runtime journal contract is now implemented: `lf` writes `meta.json` + `events.jsonl` under `.lf/runtime/runs/<run_id>/` when running in a wave worktree, and `lfd` polls those journals at 1-second intervals to emit `run.*` / `step.*` events through the EventHub. Wave attribution is filesystem-derived (sibling worktree naming), fire-and-forget, and invisible to git.

The daemon still carries its own deep execution path in `WaveExecutor`. That creates semantic drift: the CLI and the daemon each have to know how flows run, how overrides resolve, how interactive waits behave, and how results map back into run state. The longer both paths coexist, the more bugs land in the seam between them.

`lfd` should stay responsible for scheduling, queueing, worktree choice, and process supervision. It should stop being the place where loopflow execution semantics are reimplemented.

## What to build

1. **Run startup via `lf`.** When `lfd` starts an automated run, spawn `lf <flow-or-step>` in the correct worktree. The spawned process will automatically write runtime journals (via `RuntimeRun::maybe_start()` in `runtime/mod.rs`), which `lfd` already knows how to ingest.

2. **Environment injection.** Set `LFD_RUN_ID`, `LFD_WAVE_ID`, and `LFD_SESSION_ID` as env vars before exec. This lets `lf` correlate daemon-initiated runs with the daemon's own run records. The CLI's existing wave detection (filesystem-based) handles attribution; env vars add daemon correlation on top.

3. **Headless supervision.** Preserve current daemon responsibilities:
   - process lifecycle (fork, signal, reap)
   - stdout/stderr capture
   - cancellation / stop
   - exit status reconciliation
   - trigger / queue integration

4. **Run override parity.** Ensure run-scoped overrides like `flow`, `area`, and `direction` map cleanly onto the spawned CLI command or its environment. The run snapshot remains authoritative for that execution.

5. **Executor convergence.** Remove or shrink duplicate in-daemon execution logic as parity lands. Keep one implementation of flow semantics. The `WaveExecutor` should become a process supervisor around `lf`, not a parallel flow interpreter.

6. **Regression coverage.** Add tests for:
   - serialized vs parallel waves
   - queued activations
   - CI-fix / reactive runs
   - cancellation
   - failure propagation
   - run-scoped flow overrides

## Design notes

**Journal ingestion is already bidirectional.** Once daemon-spawned runs write journals the same way CLI runs do, the existing `runtime_journal::spawn()` loop picks them up automatically. The daemon gets step-level progress from its own spawned processes without parsing terminal output.

**Polling latency.** The current 1-second poll interval means daemon-spawned run visibility has ~1s lag. This is acceptable for v1 but may need to become push-based (inotify/kqueue) if latency matters for reactive scheduling.

**Malformed journal lines are skipped.** If a spawned `lf` process crashes mid-write, the daemon sees partial progress rather than no progress. This is the right tradeoff for observability.

## Open questions

- Which pieces of today's `WaveExecutor` need to survive as supervision helpers instead of disappearing entirely? (Guidance: process lifecycle, exit-status reconciliation, cancellation signaling. Not flow expansion, step resolution, or prompt assembly.)
- How do we stage the migration without breaking existing automation paths? (Guidance: dual-path first. Run the real CLI path alongside the bespoke executor, compare outcomes, swap default once parity is proven.)

## Done when

- Automated runs launched by `lfd` execute normal `lf <flow-or-step>` commands
- The daemon still owns scheduling, queueing, cancellation, and persistence
- Flow semantics no longer need to be implemented twice
- Reactive runs like CI-fix and repo-triggered activations use the same command model
- Tests prove parity between daemon-started runs and the standalone CLI path
