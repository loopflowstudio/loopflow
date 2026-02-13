# Gate review: jack-heart.remote.20260212_2130

## What was implemented

- Added Docker startup durability in `lfd`: on daemon boot, running agents are rehydrated by persisted `container_id`, missing containers are marked failed, and orphaned loopflow-managed containers are removed.
- Added durable Docker identity to agents (`agents.container_id`) via migration `003_agent_container_id.sql`, plus store/query plumbing across sqlite/postgres.
- Removed Docker fork-all rejection: `fork(select: all)` now runs in Docker using isolated fork worktrees.
- Hardened Docker credentials config from raw `host:container` strings to named allowlisted mounts (`claude`, `codex`, `gemini`, `gitconfig`, `ssh`, `gnupg`).
- Added repo image pipeline behavior in Docker executor: repo-scoped tags, fingerprint-based rebuild checks, stale sentinel handling, and default `.lf/Dockerfile` generation when missing.

## Key choices

- **Reattach over full resume:** restart recovery only restores container lifecycle tracking/log streaming and completion handling; it does not checkpoint/restore full flow execution state.
- **Typed credential mounts only:** old raw mount format is rejected at parse-time to enforce an audited allowlist.
- **Repo-scoped Docker images:** image tags are per-repo (`lfd-agent-<repo-key>:latest`) to avoid cross-repo dependency collisions.
- **Startup recovery before loops:** executor recovery runs before scheduler/background loops so stale state is reconciled before new work is scheduled.

## How it fits together

`WaveExecutor` now delegates startup recovery to the selected `AgentExecutor`. Local mode preserves existing orphaned-run failure behavior. Docker mode inspects running-agent records, repopulates active container tracking for live containers, spawns reattach tails, fails lost agents/runs, and cleans orphaned managed containers. Container IDs persist in the agents table so restart recovery can identify the right container even if process memory was lost.

## Risks and bottlenecks

- Docker image builds still shell out to `docker build` CLI, so environments with daemon access but no CLI binary can fail image build.
- Recovery cannot replay logs emitted while `lfd` was down.
- Recovery/cleanup quality depends on Docker API availability; failures are logged and startup continues.

## What's not included

- No full flow-state checkpoint/restore across daemon restarts.
- No Docker log backfill/replay for downtime periods.
- No broader Docker networking/policy hardening changes.
- No custom per-wave credential scoping.
