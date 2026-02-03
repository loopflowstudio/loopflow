# lfd as Primary Execution Path

## Problem

lfd has the infrastructure (gRPC, stores, loops, scheduler) but does not execute flows. loopflow-engine already has `tick_flow()` and agent integration, so we are paying complexity twice and still lack a working daemon. Users of the Rust program of work need lfd to become the real execution path for waves so background triggers and `RunWave` actually run steps.

## Approach

Build a single, explicit WaveExecutor in lfd that owns execution, integrates the scheduler, and bridges to loopflow-engine via a thin StoreAdapter. The executor:

- Loads a wave from storage, builds a `WaveRun`, and drives it through `tick_flow()`.
- Spawns agents via loopflow-engine, captures output into broadcast channels, and persists agent lifecycle.
- Updates wave status transitions (idle → running → waiting/idle/failed) and failure counts.
- Exposes `StreamOutput` via a per-agent broadcast receiver.
- Pauses on interactive steps and resumes via `ConnectWave` PTY.

Background loops (loop/watch/cron) become signal sources only; they enqueue non-blocking `run_wave` calls when a wave is eligible. This keeps control/execution boundaries clean while moving all real execution into one place.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Embed execution logic directly into each loop (loop/watch/cron) | Quick wiring, but duplicated logic and inconsistent state handling | Violates single execution path, hard to reason about failures and status transitions |
| Re-implement `tick_flow()` inside lfd | Avoids cross-crate dependencies | Duplicates a feature-complete engine and risks UX drift in prompts/flows |
| External worker process for execution | Strong isolation and crash containment | Adds orchestration complexity before we have a working local daemon |

## Key decisions

- Use loopflow-engine as the sole execution engine to honor “**UX invariants: prompts, flows, directions, and artifact paths must not change**”.
- Centralize all execution in a WaveExecutor to keep “**Control/execution isolation: failures in execution must not destabilize control plane**”.
- Keep gRPC as the stable surface so “**Protocol first: every project starts by validating the protocol surface**”.

## Scope

- In scope: WaveExecutor, StoreAdapter, agent spawn/output streaming, `RunWave` + `StreamOutput` + `ConnectWave` wiring, loop/watch/cron integration, status/failure handling, basic integration tests for execution and pause/resume.
- Out of scope: Container/K8s executors, auth/TLS, service install, distribution, Python CLI changes.

## Done when

- `RunWave` executes a flow end-to-end through `tick_flow()` and updates wave status to idle on success.
- Interactive steps pause in `WaveWaiting` and resume when `ConnectWave` PTY completes.
- `StreamOutput` streams agent stdout/stderr in real time.
- loop/watch/cron triggers kick off eligible waves without blocking.
- `cargo test -p lfd` passes for the new execution tests.
