# Review: Split `wave.rs` by concern + unify agent launch lifecycle

## What was implemented

Split `executor.rs` (4043 lines, single file) into a module tree organized by concern:

| Module | Responsibility | Lines |
|--------|---------------|-------|
| `executor/mod.rs` | Trait, stream helpers, shared types | ~220 |
| `executor/docker.rs` | Docker container lifecycle | ~1900 (down from ~2500 embedded) |
| `executor/local.rs` | Local process execution | ~120 |
| `executor/helpers.rs` | Prompt building, worktree management, branch advancement | ~440 |
| `executor/wave/mod.rs` | WaveExecutor: run orchestration, step dispatch, status management | ~650 |
| `executor/wave/launch.rs` | Unified agent launch lifecycle (`AgentLaunchRequest` / `AgentLaunchOutcome`) | ~80 |
| `executor/wave/fork.rs` | Fork orchestration (parallel branches, choose mode, cleanup) | ~280 |
| `executor/wave/sidecar.rs` | CI sidecar (debug run creation, execution, git push) | ~160 |
| `executor/wave/summary.rs` | Summary freshness check + internal summarize agent | ~140 |

The duplicated "build prompt -> create agent -> start -> run -> end -> emit" lifecycle that appeared in `run_step`, `run_internal_summarize`, `execute_ci_fix_agent`, and the fork branch closure is now a single `launch_agent` method in `wave/launch.rs`.

## Key choices

**Module-on-struct over free functions.** `launch_agent`, `run_fork`, `ensure_summary_fresh` etc. are `impl WaveExecutor` methods in separate files. This keeps the ownership model clear (executor owns store/scheduler/output) without introducing new traits or indirection.

**Private `wave` module with public re-export.** `mod wave` is private; `pub use wave::WaveExecutor` exposes only the struct. Internal wave concerns stay encapsulated.

**`AgentLaunchRequest` / `AgentLaunchOutcome` as plain structs.** No builder, no generics. The request carries everything needed to start an agent; the outcome returns the agent ID, exit code, and status. Simple data in, simple data out.

**Alternatives rejected:**
- Trait-based launch abstraction (`LaunchBackend`) -- adds indirection without current value
- Free function in `helpers.rs` -- lifecycle is executor behavior, not a generic helper
- Keeping duplicated launch logic while splitting modules -- leaves the main complexity untouched

## How it fits together

`WaveExecutor::execute` (in `wave/mod.rs`) drives the run loop: advance steps, dispatch to `run_step` / `run_fork` / `run_choose`. Each of these builds an `AgentLaunchRequest` and calls `launch_agent` (in `wave/launch.rs`), which handles agent creation, persistence, event emission, runner invocation, and status mapping. Fork orchestration (in `wave/fork.rs`) spawns parallel tasks that each call `launch_agent`. Summary and sidecar follow the same pattern.

`executor/mod.rs` owns the `AgentExecutor` trait (implemented by `DockerExecutor` and `LocalExecutor`), stream parsing helpers, and shared types like `CiFailure` and `EphemeralWorktree`.

## Risks and bottlenecks

- **Event ordering in forks.** Fork branches run concurrently and each call `launch_agent` independently. Agent start/end events interleave -- this matches pre-refactor behavior but could confuse consumers expecting sequential events.
- **Docker fork support.** `run_fork` still rejects `ExecutorType::Docker` with a graceful failure. This is intentional scope exclusion, not a regression.
- **`prepared_runs` unbounded growth.** `DockerExecutor::prepared_runs` (HashSet) inserts run IDs but never removes them. Minor memory concern for long-running daemons. Pre-existing, not introduced by this branch.

## What's not included

- No schema changes.
- No behavior changes to scheduler semantics, fork policy, or CI fix flow.
- No Docker fork support.
- No changes to the store layer or prompt assembly.
- The `prepared_runs` cleanup and inconsistent signal exit code defaults (`unwrap_or(1)` vs `unwrap_or(-1)`) are pre-existing issues noted for future work.

## Gate fixes applied

- Added `PartialEq, Eq` to `CiFailure` (consistency with sibling types)
- Added `#[non_exhaustive]` to `EphemeralOwnerKind` (public enum that may grow)
- Unified `short_hash` -- docker.rs now delegates to `helpers::short_hash` instead of duplicating
- Removed unused `sha2` import from docker.rs
- Moved inline `use sha2` in helpers.rs to file-level imports
- Simplified redundant empty-check + `ok_or_else` in `run_choose` to a single `let-else`
- Removed `anyhow!` import that became unused after the simplification
- Removed two restating-the-obvious comments in summary.rs
