# Review: tmux container mode — bootstrap, dispatch, status

## What was implemented

Four tmux-plugin-side deliverables for container mode:

1. **`scripts/lf-up.sh`** — one-command bootstrap that checks for `lfd` and Docker/Podman, starts the daemon if needed, polls for health (250ms intervals, 15s timeout), then opens the `lf-dev` layout. Works outside tmux (skips layout). Idempotent: focuses existing `lf-dev` window if present.

2. **Container mode dispatch** — `next` and `land` now pick a wave and run `lfq land` instead of showing "not yet implemented". Both map to `lfq land` because in wave mode, landing *is* advancing — the daemon decides whether to loop. A shared `_loopflow_container_wave_cmd` helper DRYs up the pick-wave-then-send pattern across `run`, `stop`, `logs`, `next`, and `land`.

3. **Health-aware status bar** — `generate_container_status()` now checks daemon health before querying waves. Shows `[lf: starting...]` during startup, `[lf: ! offline]` when daemon is unreachable, `[lf: idle]` when healthy but no waves. Falls back gracefully at every step.

4. **`prefix+l+u` keybinding** — sends `lf-up.sh` to the active pane. Help overlay updated to 10 bindings (was 9), popup height adjusted.

## Key choices

| Decision | Why |
|----------|-----|
| `_loopflow_container_wave_cmd` helper | Five actions shared the same pick-wave-then-send pattern. Extracted to eliminate ~30 lines of duplication. |
| `next\|land` combined case | Both dispatch to `lfq land` in container mode, and both dispatch to `lf ops $action` in lf mode. Single case with `$action` variable covers both. |
| ASCII status indicators (`!`, `...`) | Unicode (`⚠`, `...`) may not render in all terminal/font combinations. The status bar should never show garbage. |
| Health check before wave list | Prevents hanging on `lfq list --json` when daemon is down. Separates "daemon unreachable" from "daemon healthy, no waves". |
| `loopflow_has_cmd` guards | Consistent with existing patterns. `lf-up.sh` uses the helper from `helpers.sh` rather than inline `command -v`. |
| No LOOPFLOW_DIR in lf-up.sh | Already set by `helpers.sh` which is sourced. Removed redundant assignment. |

## How it fits together

```
keybinding (prefix+l+u) → dispatch "up" → tmux send-keys lf-up.sh
                                                      ↓
                                              lfd check → docker check → lfd start → health poll → layout
```

For `next`/`land`: keybinding → dispatch → `_loopflow_container_wave_cmd land` → picker → `lfq land <wave>` typed into pane.

Status bar: `loopflow-status.sh` → cache check → mode detection → `generate_container_status()` → health check → wave list → format output → cache write.

## Risks and bottlenecks

- **`lfd status` in status bar** — When daemon is unreachable, `lfd status` is called to distinguish "starting" from "offline". If `lfd` itself hangs, the status bar blocks. Mitigated by the timeout wrapper when `timeout` command is available, but macOS doesn't ship GNU `timeout` by default. The status TTL cache prevents this from happening on every render tick.

- **Wave picker in run-shell context** — The `_loopflow_container_wave_cmd` helper calls `loopflow_pick_wave`, which uses fzf in a popup when no TTY is available. This is the existing picker behavior, not new risk.

- **`tmux send-keys` for commands** — Commands are typed into the active pane rather than executed directly. This means the user sees the command and can Ctrl-C it. Downside: if the active pane is running something, the command gets typed into it. This is the existing pattern for all dispatch actions.

## What's not included

- **Rust changes** — Auto-merge verification (`land.rs`), CI polling trigger (`triggers/ci_poll.rs`), and CI status in `lfq list --json` are in the design doc but out of scope for the tmux plugin PR. Those are `lfd`/`lfq` concerns.

- **CI status in status bar** — The `⏳ CI` and `✗ CI` states from the design doc require `lfq list --json` to include a `ci_status` field. That API change is a daemon deliverable. The status bar code is ready to consume it once available.

- **Wave file deletion** — `wave/tmux/04-container-mode.md` is not deleted in this PR (gate step doesn't modify wave/).

## Test results

| Check | Result |
|-------|--------|
| Rust fmt | clean |
| Rust clippy | clean |
| Python tests | 67/67 passed |
| Shell syntax (all 3 scripts) | valid |
| tmux-review: bootstrap script | OK |
| tmux-review: help overlay (10 bindings) | OK |
| tmux-review: status script | OK |
| tmux-review: layout scripts | OK |
