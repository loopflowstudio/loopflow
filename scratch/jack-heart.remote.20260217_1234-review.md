# 01D hardening review (branch: jack-heart.remote.20260217_1234)

## What was implemented

- Added Docker coverage in CI:
  - New `docker-smoke` PR job in `.github/workflows/ci.yml`.
  - New scheduled `.github/workflows/docker-e2e-nightly.yml` for Docker restart/fork/smoke paths.
- Added shared fork-selection behavior in `engine::fork`:
  - `resolve_fork_selection` supports `one` and `prompt` semantics.
  - CLI flow execution now uses this resolver and supports `ForkSelect::One` + `ForkSelect::Prompt`.
- Improved fork execution observability:
  - CLI fork branch output is prefixed with stable branch labels (`[fork-N] ...`).
  - Daemon branch logs use per-branch output prefixes through `AgentRunContext`.
- Hardened fork cancellation and cleanup:
  - Replaced abort-based slot release with RAII slot guard (`ForkSchedulerSlotGuard`).
  - Added startup orphaned fork cleanup (`list_orphaned_fork_runs` + worktree deletion + record cleanup).
  - Added sqlite + postgres orphan-fork queries.
- Added configurable agent timeout (`executor.agent_timeout`, default 45m):
  - Config parsing + env override (`LFD_EXECUTOR_AGENT_TIMEOUT`).
  - Enforced in local and Docker executors.
- Docker image build path now uses Docker API BuildKit (`bollard build_image`) instead of shelling out to `docker build`.
- Added/updated tests covering:
  - Fork success/failure cleanup and direction merge behavior.
  - Startup orphan-fork cleanup behavior.
  - Timeout behavior in local executor.
  - Output prefix handling in executor stream parsing.
  - Empty fork branch list now fails cleanly (new regression test).

## Key choices

- **Single fork-selection resolver shared across runtimes** to keep semantics aligned and avoid CLI/daemon drift.
- **Headless `prompt` selection fails explicitly** rather than silently selecting a branch.
- **RAII scheduler slot release** instead of manual release/abort for cancellation safety.
- **Timeout as concrete config** (`executor.agent_timeout`) applied to both local and Docker executors.
- **Docker API build-only path** removes direct dependency on Docker CLI for image builds.
- **Fail-fast on empty fork branch sets** to avoid channel-capacity panic and make invalid fork config explicit.

## How it fits together

`WaveExecutor` now centralizes more reliability behavior: startup recovery first cleans orphaned fork records/worktrees, then delegates runner-specific recovery. Fork execution routes through shared selection logic and passes branch output prefixes through `AgentRunContext` into executor stream handling, so local and Docker paths format branch logs consistently. Store-level orphan queries give startup cleanup a backend-agnostic data source (sqlite + postgres).

## Risks and bottlenecks

- Docker API BuildKit context creation currently tars the repo tree directly; large repos can still make build-context generation expensive.
- Fork cancellation remains cooperative: already-running branch agents are not force-terminated by fork cancellation alone, so failure latency depends on in-flight branch duration.
- Startup orphan cleanup logs-and-continues on remove/delete failures; this is resilient but can leave stale artifacts that require later janitor cleanup.

## What's not included

- No daemon-wide checkpoint/restore of full flow execution state.
- No downtime log replay/backfill.
- No per-wave credential scoping redesign.
- No Docker network policy redesign.
- No hosted orchestration changes (remote phases 07–09).
