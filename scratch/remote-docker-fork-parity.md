# 01E: Docker Fork Parity

## Problem

Fork flows (`wave-reduce`, `wave-polish`, `wave-expand`) currently fail immediately under the Docker executor with `fork is not supported by the docker executor yet`.

This blocks remote users (Docker is the default remote runtime) from the same core workflow that works in native mode: branch fan-out, synthesis, and fail-late reporting.

Why now:
- Phase 01D established fork semantics as non-negotiable correctness behavior.
- Remote phase quality depends on parity between native and Docker execution paths.
- Current behavior forces users to avoid built-in fork flows in remote mode.

## Approach

Implement Docker fork execution as a first-class path in `WaveExecutor::run_fork`, reusing native semantics but mapping each branch to an isolated Docker worktree.

Execution model:
1. **Plan branches once** using existing `plan_fork_execution` logic.
2. **Prepare per-branch workspace identity** (`run_id + branch_index`) so each fork branch maps to a unique Docker worktree path.
3. **Serialize git mutations only** (shared-clone fetch/worktree add/reset) with the existing repo-scoped mutation lock.
4. **Launch branch agents in parallel** (one container per branch) with branch-specific worktree mounts and prefixed logs.
5. **Persist fork branch outcomes** to fork-run records and `.lf/fork-manifest.json`.
6. **Run `synthesize` after branch completion** using manifest data.
7. **Fail late**: mark run failed at end if any branch failed or synthesize failed.
8. **Always cleanup** branch worktrees + fork records, including recovery-safe cleanup after daemon restart.

Research applied:
- Reuse existing native fork contract (`manifest -> synthesize -> cleanup -> fail-late`) rather than inventing Docker-only semantics.
- Reuse existing Docker shared-clone + lock strategy to avoid git ref/worktree races.
- Follow CI matrix fail-late pattern: collect all branch results before final run verdict.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Run fork branches sequentially in one Docker worktree | Lowest implementation risk, but very slow and not true fork behavior | Violates fork intent and diverges from native parallel semantics |
| Keep rejecting fork for Docker and document limitation | No engineering effort now | Leaves a core remote capability gap and blocks wave flows users already rely on |
| Reuse one shared worktree with hard resets between branches | Fewer worktrees/paths | Branch contamination and race risk under parallel execution; hard to make restart-safe |

## Key decisions

- **Match native semantics exactly**, including manifest writing, synthesize execution, and fail-late final status.
- **Use branch-scoped Docker worktrees** (`.../worktrees/<wave>/<run>/<branch>`) to eliminate collisions between concurrent fork branches.
- **Lock only shared-clone mutation windows**; keep agent execution parallel to preserve fork throughput.
- **Treat cleanup as mandatory and idempotent**: best-effort removal plus persistent fork-run record deletion on normal completion and startup recovery paths.
- **Keep this PR scoped** to fork parity only (no Docker runtime redesign, no cross-host sync changes).

This follows the remote roadmap hardening principles:
- "Fork semantics are the same in CLI and daemon: run all branches, then synthesize."
- "Scheduler slot release and orphan-fork cleanup must be restart-safe."

## Scope

- In scope:
  - Docker fork branch worktree isolation
  - Shared-clone mutation locking during fork prep
  - Parallel branch container execution
  - Manifest + synthesize parity with native behavior
  - Fail-late run status and error messaging parity
  - Cleanup/recovery parity for fork artifacts
  - Targeted tests for success, partial failure, timeout, cleanup
- Out of scope:
  - Broader Docker executor architecture changes
  - New remote filesystem sync models
  - Non-fork executor feature work

## Done when

- Running fork flows with `executor.type = docker` no longer fails up front.
- For Docker fork runs, all branches execute, manifest is written, synthesize runs, and final status is fail-late.
- Fork worktrees and fork-run records are cleaned up on success, failure, timeout, and restart recovery.
- Verification passes:
  - `cargo test -p loopflow fork_`
  - `cargo test -p loopflow docker_`
  - CI `docker-smoke` job remains green.
