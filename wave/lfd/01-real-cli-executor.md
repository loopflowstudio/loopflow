---
linear_id: b854c1c9-b49f-47f6-be6f-381f7c7cb1b0
---
# 01: Real CLI Executor

**Finish line:** when `lfd` decides a run should start, it supervises a normal `lf <flow-or-step>` process in the correct worktree and environment instead of executing flows through a second bespoke daemon executor.

## Context

The runtime journal contract (v2) is implemented and tested:
- `journal/mod.rs` — `LfEvent` struct with `node` × `event` discriminators, `emit()` free function, `RunContext` thread-local, gitignore management
- `lfd/journal.rs` — `LfObserver` polling loop that reads JSONL under `.lf/journal/runs/<run_id>/events.jsonl` and fans events through the EventHub at 1-second intervals
- `lfd/types/event.rs` — websocket `Event` variants that map 1:1 from `LfEvent`

Wave attribution is filesystem-derived (sibling worktree naming), fire-and-forget, and invisible to git. Manual CLI runs are already observable through the shared store.

The daemon still carries its own deep execution path in `WaveExecutor` (22 files reference it). That creates semantic drift: the CLI and the daemon each have to know how flows run, how overrides resolve, how interactive waits behave, and how results map back into run state. The longer both paths coexist, the more bugs land in the seam between them.

`lfd` should stay responsible for scheduling, queueing, worktree choice, and process supervision. It should stop being the place where loopflow execution semantics are reimplemented.

### What to delete during convergence

From the v2 contract design doc (now shipped):
- `RuntimeRun` struct and its `Cell<bool>` finished guard
- `RuntimeRunMeta` / `meta.json` — the `run.started` event carries the same info
- `RuntimeEvent` enum — replaced by `LfEvent`
- `RunTarget` enum — no longer needed, flow identity comes from `flow.started`
- `map_runtime_event` — no translation needed
- All `Option<&RuntimeRun>` parameters in flow.rs and lf.rs
- `runtime/mod.rs` → replaced by `journal/mod.rs`
- `lfd/runtime_journal.rs` → replaced by `lfd/journal.rs`
- `.lf/runtime/` → replaced by `.lf/journal/`

### Escalation gap

The journal protocol supports `*.escalated` events end-to-end, but the CLI currently has no dedicated escalation error/signal type. All `anyhow::Error` paths emit `*.errored`. A dedicated escalation signal would let the CLI distinguish "I need human attention" from "something broke." This can land as part of executor convergence or as a follow-on.

## What to build

1. **Run startup via `lf`.** When `lfd` starts an automated run, spawn `lf <flow-or-step>` in the correct worktree. The spawned process will automatically write runtime journals (via `journal::emit()` in `journal/mod.rs`), which `lfd` already knows how to ingest.

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

**Journal ingestion is already bidirectional.** Once daemon-spawned runs write journals the same way CLI runs do, the existing `lfd::journal::spawn()` loop picks them up automatically. The daemon gets step-level progress from its own spawned processes without parsing terminal output.

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
