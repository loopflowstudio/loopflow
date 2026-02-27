# Tmux

## Vision

loopflow.tmux: a TPM-installable tmux surface for loopflow. Open tmux and start working immediately. Not a full-screen TUI, not Concerto feature parity, not a tmux session manager, not multi-user access controls.

- Show loopflow state without leaving the terminal.
- Start common workflows in one keypress.
- Keep a strict fallback path when deps are missing.

## Strategy

The plugin must feel reliable in two environments:

1. **lf mode (baseline/default):** no daemon required, `lf` commands run directly.
2. **container mode (advanced):** `lfd` + `lfq` power streaming and multi-wave orchestration.

Phases 01–03 (plugin skeleton, layouts, keybindings) shipped.

### Default behavior contract

- `@loopflow_mode` default: `auto`
- `auto` resolution: explicit override wins, healthy `lfq status` → container mode, otherwise lf mode
- tmux plugin load must not auto-start a container
- keybindings and status must fail soft with clear `tmux display-message`

### Quality bars

**UX:** one-line install path, keybinding feedback on every action, no silent no-op.

**Reliability:** idempotent lifecycle commands, status script bounded latency, explicit fallback when binaries missing.

**Performance:** status script target <100ms hot path, <250ms cold path, avoid repeated heavy subprocess calls per render tick.

**Security:** no auth secrets in status output or logs, no shell eval of untrusted picker text.

### Test matrix (manual)

1. Fresh machine, no `lf`/`lfq`
2. `lf` only
3. `lf` + `lfq` + running daemon
4. Docker unavailable
5. narrow terminal (<120 cols)
6. large terminal (>=200 cols)
7. slow `lfq` / daemon unavailable
8. repos with and without git remotes

## Goals

- TPM plugin installs in one line and works immediately
- Status bar shows wave state: `[lf: main]` or `[lf: 3 waves | engbot]`
- Keybindings start with `prefix+l` and cover run/stop/open/navigate
- Named layouts (`lf-dev`, `lf-swarm`) create useful workspace configurations
- Container lifecycle commands (`lfd install/start/stop/status/update/uninstall`) work from keybindings
- `lf up` bootstraps a usable workspace in one command

## Risks

- **Status jitter:** cache with TTL + stale marker.
- **Mode confusion:** include active mode marker in status and help overlay.
- **Command drift:** centralize dispatch in helpers; no duplicated command strings.
- **Container lifecycle edge cases:** make every command safe to retry.

## Metrics

- Install + first successful action in <2 minutes
- Status updates within one `status-interval`
- All keybindings operate (or fail-soft) in both modes
- `lf up` gives a usable workspace on first run
