# 04: Container Mode

`lfd` lifecycle management for the container. One-command setup, auto-discovery, clean teardown.

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

### Container configuration

```yaml
# ~/.lf/config.yaml
repos_root: ~/src          # default, auto-discovered repos
```

Container mounts:
- `repos_root` → `/repos` (read-only for discovery, git status, wave config)
- Per-wave executor worktrees get read-write bind mounts scoped to `repos_root/<repo>.<wave>`

### Auto-discovery

On startup, lfd scans `repos_root` for git repositories. No explicit `lfq repo add`. If it's a git repo under `repos_root`, lfd knows about it.

### `lf up` — the one-command entry point

```bash
lf up              # start container if not running, open lf-dev layout
lf up --detach     # start container only, no tmux layout
```

`lf up` checks if lfd is running. If not, runs `lfd start`. Then opens the default tmux layout for the current repo.

### Container naming

Container name derives from the hostname or a user-configurable name: `lf-<hostname>`. Multiple machines get isolated containers.

## Constraints

- `lfd` commands work without tmux installed (container lifecycle is independent of the tmux plugin).
- Container mode requires Docker or Podman. Detect which is available, prefer Docker.
- `lfd start` is idempotent — running it twice doesn't create duplicate containers.
- `lfd stop` is graceful — signals the daemon, waits for running agents to checkpoint, then stops.

## Validation

```bash
lfd install
lfd start
lfd status
lfq list
lfd stop
lfd status
```

## Done when

- `lfd install` pulls the image and runs auth onboarding
- `lfd start/stop` manage the container lifecycle
- `lfd status` shows container state and wave count
- `lf up` starts the container and opens a tmux layout
- Auto-discovery finds repos under `repos_root`
- `lfd uninstall` removes everything cleanly
