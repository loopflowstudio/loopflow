# Simplification Opportunities

## Product intent
Loopflow wants to be a reliable automation runtime for coding work: define waves, run flows, and recover cleanly when something fails. The core user promise is predictable execution across local CLI, daemon APIs, and long-running autonomous loops. The architecture should make wave state, workspace state, and execution state explicit and observable.

## Opportunity 1: Make storage and daemon execution model match
**Misalignment**: `lfd` is async and event-driven, but persistence is modeled as a synchronous trait.

**Symptom**:
- `RunStore` is synchronous (`lfd/store/mod.rs`), so async handlers call `spawn_blocking` through `run_store` (`lfd/http/mod.rs`).
- `PostgresStore` has runtime bridging (`block_on`, `block_in_place`) and a dedicated comment acknowledging this compromise (`lfd/store/postgres.rs`).
- Route handlers and system endpoints repeatedly bounce between async and blocking boundaries.

**Realignment**: Move to an async store boundary (`AsyncRunStore`) and make backends native to it (or keep one backend and remove the second until needed). Keep retry/timeout policy inside the store layer.

**Cascade**:
- Removes `run_store` + many `spawn_blocking` wrappers in HTTP routes.
- Deletes Postgres runtime-in-runtime bridging code.
- Makes cancellation, timeout, and observability behavior consistent for all DB operations.

## Opportunity 2: Promote wave workspace to a first-class domain object
**Misalignment**: The product model is “a wave has a stable workspace,” but code passes raw `String` paths and branch names around per run.

**Symptom**:
- `WaveRun` stores `worktree`/`branch` strings; recovery logic re-derives and repairs them in multiple places.
- `ensure_wave_worktree` (`lfd/executor.rs`) and `resolve_wave_work_dir*` (`lfd/http/routes/waves.rs`) duplicate workspace resolution/recreation behavior.
- Repeated `main_repo_root(...).unwrap_or_else(...)`/`worktree_path(...)` patterns across executor, ops, and routes.

**Realignment**: Introduce a `WaveWorkspace` type (canonical repo root + wave key + branch + path) with a single resolver/validator used by executor, ops, and HTTP handlers. Treat fork/CI-fix paths as explicit ephemeral workspace variants.

**Cascade**:
- Deletes duplicated path/branch repair helpers.
- Reduces stale-worktree edge cases and rename drift.
- Simplifies janitor/recovery logic because ownership and lifecycle become explicit.

## Opportunity 3: Split orchestration from runtime/backends in `lfd/executor`
**Misalignment**: One module currently owns multiple product concerns: flow state transitions, prompt assembly, git lifecycle, PR automation, local runtime, docker runtime, CI-fix sidecars, and cleanup.

**Symptom**:
- `lfd/executor.rs` is ~4k lines and contains both backend implementations and high-level wave orchestration.
- Functions like `build_step_prompt`, `auto_create_pr`, `advance_branch`, fork handling, and CI-fix execution live beside container/process runtime code.
- Small policy changes force edits in a high-risk, high-coupling file.

**Realignment**: Separate into focused components:
- `RunStateMachine` (flow progression only)
- `AgentBackend` (local/docker execution only)
- `WorkspaceService` (worktree lifecycle)
- `PostRunPolicy` (commit/PR/next-branch)
- `SidecarService` (CI fix)

**Cascade**:
- Faster, safer changes with smaller blast radius.
- Better failure isolation and targeted retries.
- Clearer observability boundaries (state transition vs backend execution vs git side effects).

## Aligned areas
- `engine/flow.rs` maps product concepts (step, fork, flow action) to typed structures cleanly; keep this as the orchestration contract.
- `lfd/scheduler.rs` is simple and explicit about slot ownership; preserve that clarity.
- `engine/worktrees.rs` already centralizes important git/worktree primitives; use it as the base for a first-class workspace model instead of adding more path helpers elsewhere.
