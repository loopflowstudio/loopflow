# Sandboxed Agent Execution — 01D Hardening (Current State)

## Objective

Make Docker-backed remote execution boring and trustworthy before later remote phases (05–09), with aligned fork behavior across CLI and daemon and strong recovery/cleanup guarantees.

## Shipped in this branch

- Docker coverage promoted into CI:
  - Required PR `docker-smoke` job.
  - Scheduled nightly `docker-e2e-nightly` workflow.
- Shared fork-selection resolver now used by runtimes:
  - Supports `ForkSelect::One` deterministic selection.
  - Supports `ForkSelect::Prompt` in TTY; fails fast in headless mode.
- Fork log readability improved:
  - Stable per-branch prefixes in CLI and daemon output.
- Cancellation and cleanup hardened:
  - Scheduler slot release now guarded by RAII (`ForkSchedulerSlotGuard`).
  - Startup orphan-fork cleanup added (sqlite + postgres).
- Agent execution timeout added:
  - `executor.agent_timeout` (default 45m), with `LFD_EXECUTOR_AGENT_TIMEOUT` override.
  - Enforced for local and Docker executors.
- Docker image build path switched to Docker API BuildKit (`bollard build_image`) instead of shelling out to `docker build`.
- Integration/regression coverage expanded for fork success/failure cleanup, direction merge behavior, startup orphan cleanup, timeout handling, prefixed stream parsing, and empty-fork fail-fast behavior.

## Locked decisions

- Keep one shared fork-selection implementation to avoid CLI/daemon drift.
- Headless `prompt` never auto-selects; it returns a clear error.
- Scheduler slot release must be cancellation-safe by construction (RAII guard).
- Timeout is an explicit operator config (`executor.agent_timeout`), not implicit watchdog behavior.
- Docker image build remains API-first; no dual CLI/API build path.

## Remaining follow-up work (outside 01D)

- Improve Docker build-context performance for large repos (current tar creation can be expensive).
- Add optional hard-stop cancellation for already-running fork branches to reduce failure latency.
- Add periodic janitor cleanup/alerting for orphan cleanup failures that are currently log-and-continue.

## Explicitly out of scope for 01D

- Full flow checkpoint/restore across daemon restarts.
- Downtime log replay/backfill.
- Per-wave credential scoping redesign.
- Docker network policy redesign.
- Hosted orchestration work for remote phases 07–09.
