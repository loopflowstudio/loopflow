# Tmux

loopflow.tmux: a TPM-installable tmux surface for loopflow. Status, layouts, and action bindings first. Container lifecycle second.

## Product intent

Open tmux and start working immediately.

- Show loopflow state without leaving the terminal.
- Start common workflows in one keypress.
- Keep a strict fallback path when deps are missing.

The plugin must feel reliable in two environments:

1. **lf mode (baseline/default):** no daemon required, `lf` commands run directly.
2. **container mode (advanced):** `lfd` + `lfq` power streaming and multi-wave orchestration.

## Scope and non-goals

### In scope

- TPM plugin entrypoint (`loopflow.tmux`)
- status segment with low-latency mode-aware rendering
- named layouts (`lf-dev`, `lf-swarm`)
- mode-aware keybindings
- container lifecycle commands (`lfd install/start/stop/status/update/uninstall`)
- `lf up` one-command bootstrap

### Out of scope

- Full-screen TUI app
- Concerto feature parity
- tmux session manager integration
- multi-user access controls

## Default behavior contract

- `@loopflow_mode` default: `auto`
- `auto` resolution:
  1. explicit override wins (`lf` or `container`)
  2. healthy `lfq status` => container mode
  3. otherwise lf mode
- tmux plugin load must **not** auto-start a container.
- keybindings and status must fail soft with clear `tmux display-message`.

## Remaining artifacts

- `scripts/lfd` (phase 04)
- `scripts/lf-up.sh` (phase 04)

## Quality bars

### UX
- one-line install path
- keybinding feedback on every action
- no silent no-op

### Reliability
- idempotent lifecycle commands
- status script bounded latency
- explicit fallback when binaries missing

### Performance
- status script target <100ms hot path, <250ms cold path
- avoid repeated heavy subprocess calls per render tick

### Security
- no auth secrets in status output or logs
- no shell eval of untrusted picker text

## Test matrix (manual)

1. Fresh machine, no `lf`/`lfq`
2. `lf` only
3. `lf` + `lfq` + running daemon
4. Docker unavailable
5. narrow terminal (<120 cols)
6. large terminal (>=200 cols)
7. slow `lfq` / daemon unavailable
8. repos with and without git remotes

## Rollout

Phases 01–03 (plugin skeleton, layouts, keybindings) shipped. Next:

1. Dogfood plugin with container mode disabled by default.
2. Ship 04 and enable `auto` container detection.
3. Harden with failure telemetry and operator docs.

## Risks and mitigations

- **Status jitter:** cache with TTL + stale marker.
- **Mode confusion:** include active mode marker in status and help overlay.
- **Command drift:** centralize dispatch in helpers; no duplicated command strings.
- **Container lifecycle edge cases:** make every command safe to retry.

## Success criteria

- install + first successful action in <2 minutes
- status updates within one `status-interval`
- all keybindings operate (or fail-soft) in both modes
- `lf up` gives a usable workspace on first run
