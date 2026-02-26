# 04: Container Mode

`lfd` lifecycle management for containerized loopflow with predictable startup, health checks, and clean teardown.

## What to build

### lfd lifecycle commands

`lfd` is a thin CLI (shell script or small binary) that manages the container:

```bash
lfd install          # pull image + auth onboarding (see auth wave 04)
lfd start            # start container, mount repos_root readonly
lfd stop             # stop container
lfd status           # running? uptime? wave count?
lfd update           # pull latest image, restart
lfd uninstall        # remove image, data, config
```

Command behavior contract:

- `install`: safe to re-run; no duplicate install artifacts.
- `start`: idempotent; if already running, return success + status summary.
- `stop`: graceful first, force only on timeout.
- `status`: machine-readable output option (`--json`) plus human summary.
- `update`: pull + restart with health gate.
- `uninstall`: explicit confirmation for destructive cleanup.

### Container configuration

```yaml
# ~/.lf/config.yaml
repos_root: ~/src          # default, auto-discovered repos
container_name: lf-${HOST}
runtime: auto              # docker|podman|auto
```

Container mounts:
- `repos_root` → `/repos` (read-only for discovery, git status, wave config)
- Per-wave executor worktrees get read-write bind mounts scoped to `repos_root/<repo>.<wave>`

### Auto-discovery

On startup, lfd scans `repos_root` for git repositories. No explicit `lfq repo add`. If it's a git repo under `repos_root`, lfd knows about it.

Discovery constraints:

- ignore hidden/system dirs by default
- cap traversal depth
- bounded scan timeout with progress log
- rescan command for manual refresh

### `lf up` — the one-command entry point

```bash
lf up              # start container if not running, open lf-dev layout
lf up --detach     # start container only, no tmux layout
```

`lf up` checks if lfd is running. If not, runs `lfd start`. Then opens the default tmux layout for the current repo.

Mac-first UX target:

- first-time `lf up` should guide through install prerequisites
- when runtime missing, provide exact next command
- avoid silent fallback to broken local state

### Container naming

Container name derives from the hostname or a user-configurable name: `lf-<hostname>`. Multiple machines get isolated containers.

### Health/readiness contract

`lfd start` is not “done” until:

1. container process running
2. lfd `/health` endpoint healthy
3. auth and storage initialized
4. `lfq status` succeeds

Expose readiness timeout and diagnostics.

## Constraints

- `lfd` commands work without tmux installed (container lifecycle is independent of the tmux plugin).
- Container mode requires Docker or Podman. Detect which is available, prefer Docker.
- `lfd start` is idempotent — running it twice doesn't create duplicate containers.
- `lfd stop` is graceful — signals the daemon, waits for running agents to checkpoint, then stops.
- No destructive cleanup on plain `stop`.

## Failure handling

- runtime missing: clear actionable error
- port collision: auto-select new port or show exact conflicting process
- auth not configured: continue startup but mark degraded and provide `lfq auth` next step
- stale container: auto-recover or print recovery command

## Security and data handling

- no secrets printed in `status` output
- all config/token paths explicit and documented
- teardown command distinguishes:
  - stop runtime
  - remove container
  - remove persistent data

## Validation

```bash
lfd install
lfd start
lfd status
lfq list
lfd stop
lfd status
```

Manual checks:

1. fresh machine: install → start → status → up
2. repeated start/stop cycles
3. daemon crash + restart recovery
4. Docker unavailable path
5. Podman path (if supported)
6. uninstall + clean re-install

## Done when

- `lfd install` pulls the image and runs auth onboarding
- `lfd start/stop` manage the container lifecycle
- `lfd status` shows container state and wave count
- `lf up` starts the container and opens a tmux layout
- Auto-discovery finds repos under `repos_root`
- `lfd uninstall` removes everything cleanly
- readiness/health checks are enforced, not best-effort
