# 01C: Sandboxed Agent Hardening (Current State)

Close Docker executor hardening for production usage. This file is the single source of truth for what already landed and what is still open.

## Landed on this branch

### Docker restart durability

- Persisted `agents.container_id` so Docker identity survives daemon restarts.
- Startup recovery now runs before scheduler/background loops:
  - rehydrates running Docker agents into in-memory tracking
  - reattaches log/exit/sync tail for recovered containers
  - fails runs when expected containers are missing
  - removes orphaned loopflow-managed containers

### Fork isolation in Docker

- Removed Docker-only rejection for `fork(select: all)`.
- Fork branches now run in parallel containers with per-fork worktrees inside the same repo volume.
- Shared clone mutations stay lock-guarded; fork execution stays concurrent.

### Credential mount hardening

- Replaced raw `host:container` mount strings with typed, named mounts.
- Enforced fixed allowlist (`claude`, `codex`, `gemini`, `gitconfig`, `ssh`, `gnupg`).
- Unknown mount names now fail config parsing.
- All credential mounts remain read-only.

### Image lifecycle hardening

- Added repo-scoped image tags (`lfd-agent-<repo-key>:latest`) to avoid cross-repo collisions.
- Added fingerprint-based rebuild checks for `.lf/Dockerfile`, `.lf/env-setup.sh`, and base image ref.
- Added stale sentinel support (`.lf/.docker-stale`).
- Added default `.lf/Dockerfile` generation when missing.
- Added per-image build coordination so concurrent waves do not duplicate builds.

## Remaining to close 01C

### 1) Docker CI coverage

Add automated coverage for Docker paths still missing in CI:

- PR smoke job (daemon connectivity, helper container run, volume lifecycle, mount resolution)
- Nightly Docker e2e (parallel waves, fork fanout, cancel/cleanup, image rebuild trigger paths)

### 2) Decide build backend policy

Current image build path shells out to `docker build` CLI. Decide whether to:

- keep CLI dependency and document it as required runtime tooling, or
- move image builds fully to Docker API/BuildKit integration.

## Non-goals (unchanged)

- Full flow-state checkpoint/restore across daemon restarts
- Docker log replay/backfill for downtime windows
- Per-wave credential scoping
- Docker network policy redesign

## Stage-close checklist

```bash
# Existing broad suites
cargo fmt --all
cargo clippy --all -- -D warnings
cargo test --all
uv run pytest python/tests/
tests/e2e/test_smoke.sh

# Docker-specific coverage (to add to CI)
cargo test -- --ignored                 # Docker smoke tests
./tests/e2e/test_docker_smoke.sh        # Docker e2e smoke
```
