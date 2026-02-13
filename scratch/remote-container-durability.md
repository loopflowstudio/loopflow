# Docker Container Durability & Startup Recovery

## Scope

Improve `lfd` Docker-mode durability across daemon restarts without attempting full flow-state resume.

## Problem

Before this change, Docker agent tracking lived only in an in-memory `active` map (`agent_id -> container_id`).
When `lfd` restarted:

- running containers became invisible to `lfd`
- logs stopped routing to OutputHub/Concerto
- runs were blanket-marked failed by orphan handling
- old containers kept running and could accumulate as leftovers

## Current behavior

Startup recovery now runs before scheduler/background loops begin.

### 1) Rehydrate running Docker agents

For agents marked Running:

1. Check whether the recorded/container-associated Docker container still exists and is running.
2. If running, repopulate the in-memory `active` map.
3. Spawn a lightweight reattach tail that:
   - streams logs
   - waits for container exit
   - syncs worktree back
   - records exit/final status
   - removes container from Docker + active map
4. If container is missing, mark the agent failed and fail the related run with a restart-loss error.

Local-process mode keeps existing orphan-failure behavior; only Docker mode rehydrates.

### 2) Cleanup orphaned loopflow containers

After rehydration, `lfd` enumerates containers with:

- `io.loopflow.managed=true`

Any such container not present in the recovered active set is stopped/removed and logged.

## Data model and metadata

### Agents table

Added nullable `container_id TEXT` (`003_agent_container_id.sql`) so Docker identity survives daemon restarts.

### Docker labels

Managed containers now include:

- `io.loopflow.managed=true`
- `io.loopflow.agent-id=<agent_id>`
- `io.loopflow.wave-id=<wave_id>`
- `io.loopflow.wave-run-id=<wave_run_id>`

## Key decisions

- **Reattach, not full resume**: recover container lifecycle and result collection; do not serialize/restore executor step iterator state.
- **Recover before scheduling**: avoid duplicate work while stale state is being reconciled.
- **Dedicated `container_id` field**: keep `pid` semantics clean for local mode.
- **Labels + durable IDs**: support robust container identification beyond naming conventions.

## Known limitations

- No mid-flow resume after daemon restart; future stimuli restart flow execution.
- Logs emitted while `lfd` is down are not replayed.
- Recovery/cleanup depend on Docker API availability (failures are logged; daemon continues startup).

## Validation run

- `cargo fmt --all`
- `cargo clippy --all -- -D warnings`
- `cargo test --all`
- `uv run pytest python/tests/`
- `tests/e2e/test_smoke.sh`

## Out of scope

- Full execution-state checkpoint/restore
- Docker log persistence/replay during downtime
- Volume lifecycle redesign
- User-facing CLI surface changes for this feature
