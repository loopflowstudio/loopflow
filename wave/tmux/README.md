# Tmux

loopflow.tmux — an installable tmux plugin. Named layouts, keybindings, status bar, and container-friendly daemon lifecycle. The "barebones Concerto" for terminal users.

## Vision

A developer opens a tmux session and loopflow is already there. Status bar shows running waves. Prefix keys start and tail agents. Layouts arrange panes for the right workflow. No setup beyond adding one line to `.tmux.conf`.

Two modes serve different needs:

- **lf mode** (default): `lf` commands run directly in tmux panes. No daemon, no container. Works anywhere `lf` is installed. This is the entry point — everything else is optional.
- **Container mode**: `lfd install` pulls a container with lfd + agent harnesses. `lf up` starts it. `lfq` queries the running daemon. Streaming output, multi-wave orchestration, session interaction. The Linux-native equivalent of Concerto.

The container is the deployment unit. `lfd` (the CLI command) is a lifecycle manager for the container, not the daemon binary. Running raw `lfd serve` is possible but not paved.

### Not here

- TUI / ratatui application (tmux panes + shell scripts, not a terminal app)
- Session persistence beyond what lfd provides
- Integration with tmux session managers (tmux-sessionizer, etc.)
- Concerto feature parity (quote-replies, action buttons, etc.)

## Goals

- TPM-installable in one `.tmux.conf` line
- Status bar shows wave count and health at a glance
- Named layouts cover common workflows without custom config
- `lf` mode works immediately with zero daemon setup
- Container mode unlocks streaming and multi-wave on Linux

## Phase boundaries

- **01-tpm-skeleton**: Plugin entry point, TPM registration, status segment, option keys, basic keybindings.
- **02-named-layouts**: `lf-dev`, `lf-swarm`, `lf-flow` layouts callable via keybinding or `lf tmux`.
- **03-keybindings**: Wave action bindings (run, logs, status, stop, land, next) with configurable prefix.
- **04-container-mode**: `lfd install/start/stop/status/update` lifecycle. Container auto-discovery of repos from `repos_root`. `lf up` as the one-command entry point.

## Risks

- **PATH at plugin load time.** If `lf`/`lfq` aren't in `$PATH` when tmux sources the plugin, status commands silently fail. The status script must handle missing binaries gracefully — show `--` not an error.
- **Status segment latency.** tmux `status-interval` defaults to 15s. If the status script shells out to `lfq`, it needs to be fast. Consider caching to a file that lfd updates via event, or reading git state directly for lf mode.
- **Layout assumptions.** Layout scripts assume minimum terminal dimensions. Degrade gracefully (fewer panes) on small terminals. Document minimums.
- **Container cleanup.** Container mode creates persistent state. `lfd stop` must clean up reliably. `lfd uninstall` removes everything.
- **Two-mode complexity.** Supporting both lf mode and container mode means every keybinding needs to know which mode it's in. Keep the dispatch simple — check if lfd is running, use `lfq` if yes, `lf` if no.

## Metrics

- `set -g @plugin 'loopflowstudio/loopflow.tmux'` + TPM install works on a fresh machine
- Status bar updates within one `status-interval` of a wave state change
- All three layouts open without errors on a 200x50 terminal
- `lfd install && lf up` starts the container and drops into a tmux layout
- lf mode keybindings work with no daemon running
