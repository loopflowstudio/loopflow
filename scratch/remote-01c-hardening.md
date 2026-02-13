# 01C: Sandboxed Agent Hardening (Current State)

This doc tracks closeout status for 01C and the remaining production hardening work.

## Landed on this branch

### Docker restart durability

- Persisted `agents.container_id` so Docker-backed agents survive daemon restarts.
- Daemon startup now recovers running containers before scheduler/background loops.
- Recovery rehydrates in-memory tracking, reattaches lifecycle/log tailing, fails runs when expected containers are missing, and removes loopflow-managed orphan containers.

### Fork isolation in Docker

- Removed Docker-only rejection for `fork(select: all)`.
- Fork branches now execute in parallel containers with per-fork worktree isolation.
- Shared clone mutation remains serialized while fork execution remains concurrent.

### Credential mount hardening

- Replaced raw `host:container` mount strings with typed named mounts.
- Enforced allowlist: `claude`, `codex`, `gemini`, `gitconfig`, `ssh`, `gnupg`.
- Unknown mount names now fail config parsing.
- Credential mounts remain read-only.

### Image lifecycle hardening

- Added repo-scoped image tags (`lfd-agent-<repo-key>:latest`) to prevent cross-repo collisions.
- Added fingerprint-based rebuild triggers for `.lf/Dockerfile`, `.lf/env-setup.sh`, base `FROM`, and `.lf/.docker-stale`.
- Added default `.lf/Dockerfile` generation when missing.
- Added per-image build coordination to prevent duplicate concurrent builds.

## Remaining to close 01C

### 1) Add Docker CI coverage

- PR smoke coverage for daemon connectivity, helper container run, volume lifecycle, and mount resolution.
- Nightly Docker e2e for parallel waves, fork fanout, cancel/cleanup, and image rebuild triggers.

### 2) Decide build backend policy

Current implementation shells out to `docker build`.

Decision needed:
- Keep Docker CLI as explicit runtime dependency, or
- Move image builds to Docker API/BuildKit.

## Known limits (accepted in this stage)

- No full flow-state checkpoint/restore across daemon restarts.
- No downtime log replay/backfill.
- No per-wave credential scoping.
- No Docker network policy redesign.

## Validation checklist

```bash
cargo fmt --all
cargo clippy --all -- -D warnings
cargo test --all
uv run pytest python/tests/
tests/e2e/test_smoke.sh

# Docker-specific suites (to wire into CI)
cargo test -- --ignored
./tests/e2e/test_docker_smoke.sh
```
