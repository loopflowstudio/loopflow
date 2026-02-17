# Sandboxed Agent Execution — 01D Hardening Plan

## Problem

Phase 01 proved that loopflow can run agents in Docker with repo volumes, restart recovery, and credential allowlisting. What remains is reliability debt that will slow every remote phase after this.

Who benefits:
- **Remote users** get predictable runs instead of hung agents and orphaned worktrees.
- **Contributors** get CI coverage for Docker paths before shipping regressions.
- **Operators** get readable fork logs and deterministic cleanup after partial failure.

Why now:
- Phases 05–09 depend on Docker execution being boring and trustworthy.
- Today, critical Docker/fork behavior is only partially covered in CI and integration tests.

## Approach

Ship one focused hardening tranche (01D) that closes the eight remaining gaps with shared primitives instead of one-off fixes.

1. **Make Docker correctness a required signal in CI**
   - Add a required `docker-smoke` PR job in `.github/workflows/ci.yml`.
   - Add a scheduled `docker-e2e-nightly.yml` workflow for restart/fork/end-to-end Docker paths.

2. **Unify fork execution semantics across CLI and daemon**
   - Add a shared fork selection resolver used by both runtimes.
   - Implement `ForkSelect::One` and `ForkSelect::Prompt` in CLI mode.
     - `one`: deterministic branch selection.
     - `prompt`: interactive select in TTY, explicit error in headless mode.
   - Prefix fork branch logs with stable branch labels in both CLI and daemon.

3. **Make fork cleanup cancellation-safe**
   - Replace hard `handle.abort()` shutdown with cooperative cancellation.
   - Introduce a guard that always releases scheduler slots on drop.
   - Add startup orphan-fork cleanup driven by store queries valid for sqlite + postgres.

4. **Commit to Docker API image builds (BuildKit via Bollard)**
   - Remove direct `docker build` shell dependency from agent image build path.
   - Keep repo-scoped tags + fingerprint invalidation, but build through Docker API streams.

5. **Add explicit agent execution timeout**
   - New config: `executor.agent_timeout` (duration, default 45m).
   - Apply to both local process and Docker executor waits.
   - On timeout: terminate agent/container, mark run failed, emit structured timeout reason.

6. **Lock confidence with integration tests first**
   - Add integration tests for: successful fork, partial fork failure with cleanup, direction merge behavior.
   - Reuse `loopflow-test-support::TestRepo` for deterministic repository setup.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Patch each remaining item independently | Low coordination cost, but duplicates logic and leaves fork behavior divergent | Creates long-term maintenance drag and inconsistent user behavior |
| Re-architect now around Kubernetes/containerd | Maximum isolation and scheduling flexibility | Massive scope jump that delays remote roadmap without immediate user value |
| **Chosen: 01D hardening tranche with shared fork/runtime primitives + Docker API builds** | Higher upfront integration work | Best path to make remote execution trustworthy and keep future phases fast |

## Key decisions

- **"Bold over safe"**: remove the Docker CLI build dependency now instead of carrying two build paths.
- **"Concrete over abstract"**: timeout is a real config (`executor.agent_timeout`, default **45 minutes**), not a vague “watchdog later.”
- **"Decisions over options"**: headless `ForkSelect::Prompt` fails fast with a clear error; it does not silently pick a branch.
- Fork slot release is guaranteed by RAII-style guard semantics so cancellation cannot leak scheduler capacity.
- Orphaned fork worktree cleanup runs at daemon startup from persisted fork-run state, not filesystem heuristics alone.
- Success scenario we optimize for: users can run parallel forks overnight and wake up to clean logs, clean worktrees, and deterministic status.
- Failure scenario we prevent: one stuck branch or aborted task deadlocks scheduler slots and leaves hidden garbage until manual cleanup.

## Scope

- In scope:
  - PR Docker smoke job and nightly Docker e2e workflow
  - Fork integration tests (success, partial failure, direction merge)
  - CLI support for `ForkSelect::One` and `ForkSelect::Prompt`
  - Fork log prefixing in parallel branch output
  - Cancellation-safe fork cleanup and scheduler slot release guarantees
  - Startup orphaned fork worktree cleanup across sqlite/postgres stores
  - Docker API/BuildKit image build backend
  - Configurable agent execution timeout for local + Docker executors

- Out of scope:
  - Full flow-state checkpoint/restore across daemon restarts
  - Downtime log replay/backfill
  - Per-wave credential scoping
  - Docker network policy redesign
  - Hosted SaaS orchestration (remote phases 07–09)

## Done when

- `docker-smoke` is a required PR check and is green on at least one non-trivial PR.
- A scheduled nightly workflow runs Docker e2e coverage and reports pass/fail status.
- New integration tests cover fork success, fork partial failure cleanup, and direction merge behavior.
- CLI `ForkSelect::One` works deterministically; `ForkSelect::Prompt` works in TTY and fails clearly in headless mode.
- Parallel fork logs are prefixed per branch, making interleaved output readable.
- Forced cancellation does not leak scheduler slots (verified by test).
- Startup cleanup removes orphan fork worktrees left by interrupted runs (verified for sqlite + postgres stores).
- Agent timeout kills hung runs and records explicit timeout errors in run state.
