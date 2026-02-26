# Container Mode: tmux as the front door

## Problem

The tmux plugin shipped with status, layouts, and keybindings (phases 01–03). It auto-detects container mode and dispatches commands differently based on mode. But the integration is shallow: `next` and `land` show "not yet implemented" in container mode, there's no way to bootstrap a container environment from tmux, and the status bar doesn't distinguish "daemon healthy" from "daemon missing."

Users who want container-mode orchestration (multi-wave, streaming logs, persistent daemon) currently have to manage `lfd install && lfd start` out-of-band, then hope the tmux plugin detects it. There's no guided path from "I have tmux" to "I have a running loopflow environment."

## Approach

Three deliverables, each independently useful:

### 1. `scripts/lf-up.sh` — one-command bootstrap

A shell script (callable as `lf up` or from keybinding) that gets you from zero to working:

```bash
scripts/lf-up.sh [--detach]
```

Behavior:
1. Check if `lfd` binary exists. If not: print install instructions, exit 1.
2. Check `lfd status`. If not running: run `lfd start`, wait for health.
3. Health gate: poll `lfq status` with 250ms intervals, 15s timeout. Show progress via `tmux display-message` or stdout.
4. If `--detach`: exit 0 (container running, no layout).
5. Otherwise: open `lf-dev` layout for the current repo.

First-run experience for a user with Docker installed:
```
$ lf up
loopflow: starting lfd...
loopflow: waiting for health... (3s)
loopflow: ready — opening lf-dev layout
```

First-run for a user without Docker:
```
$ lf up
loopflow: docker not found — install Docker Desktop: https://docker.com/get-started
```

The script works **outside tmux** too (skips layout, just starts daemon). This means `lf up` can be the first command in a user's shell profile or in CI.

### 2. Complete container mode dispatch

Wire up the three remaining actions in `loopflow_dispatch()`:

| Action | Current (container) | New behavior |
|--------|-------------------|--------------|
| `next` | "not yet implemented" | `lfq next <wave>` via picker |
| `land` | "not yet implemented" | `lfq land <wave>` via picker |
| `logs` | works | no change |
| `run` | works | no change |
| `stop` | works | no change |

Also add a new action:

| Action | Key | Behavior |
|--------|-----|----------|
| `up` | `u` | Run `scripts/lf-up.sh` |

### 3. Health-aware status bar

Enhance `loopflow-status.sh` to show daemon health state:

| State | Output |
|-------|--------|
| daemon healthy, waves active | `[lf: 3 waves ▸ engbot]` |
| daemon healthy, no waves | `[lf: idle]` |
| daemon starting | `[lf: starting…]` |
| daemon unhealthy/unreachable | `[lf: ⚠ offline]` |
| no lfd binary | `[lf: main]` (lf mode, as today) |

The `starting…` state triggers when `lfd status` returns a "starting" state or when health check fails but process exists. The `⚠ offline` state triggers when `lfq status` fails and `lfd` binary exists (daemon should be running but isn't).

Cache all states — the status script is called on every render tick.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Build `lf up` into the Rust `lf` binary | Single binary distribution, no shell fragility | `lf up` opens a tmux layout — it's fundamentally a tmux concern. The Rust binary shouldn't know about tmux panes. Keep workspace setup in the tmux plugin. |
| Auto-start daemon on plugin source | Zero-action UX | Violates "plugin load must not auto-start a container" from wave README. Side effects on `source` are hostile. |
| Separate `lfd-tmux` wrapper binary | Clean separation | Over-engineered. Shell scripts are the right tool for glue between tmux and CLI binaries. The plugin is already pure shell. |
| Skip health gate, trust `lfd start` | Simpler | `lfd start` returns before health check passes. Opening a layout before the daemon is ready means broken first experience. The 15s gate is worth it. |

## Key decisions

**`lf up` is a tmux plugin script, not an `lf` subcommand.** The `lf` binary runs prompts and flows. `lf up` orchestrates tmux layouts and daemon lifecycle — that's tmux-plugin territory. If users want it on their PATH, they can symlink or alias.

**Health gate uses polling, not blocking.** `lfd start` on launchd/systemd returns immediately. The plugin polls `lfq status` at 250ms intervals (matching `@loopflow_status_timeout_ms`) for up to 15s. This gives visible progress rather than a frozen terminal.

**`prefix+l+u` keybinding for `up`.** One keypress to ensure the daemon is running and open a layout. Idempotent: if already running, just opens the layout. If layout already exists, focuses it.

**Status bar degrades, never errors.** Unknown states render as `[lf]`. Parse failures fall back to cached text. Timeouts return stale cache with `~` marker. No blank status bar, ever.

**Container mode `next` and `land` use picker.** Both require selecting a wave first (which wave to iterate? which wave to land?). Use the same `loopflow_pick_wave` picker that `run`, `stop`, and `logs` already use.

## Scope

### In scope

- `scripts/lf-up.sh` with daemon bootstrap, health gate, and layout open
- Container mode dispatch for `next` (`lfq next`) and `land` (`lfq land`)
- New `up` action and `prefix+l+u` keybinding
- Health-aware status bar (healthy/starting/offline states)
- Graceful error messages for missing Docker, missing `lfd`, missing `lfq`
- Update `tmux-review.py` with structural tests for new scripts and keybindings
- Update help overlay with `u` binding

### Out of scope

- `lfd update` command (daemon concern, not tmux plugin)
- Auto-discovery of repos under `repos_root` (daemon concern)
- `lfd uninstall` from tmux (destructive, belongs in CLI)
- Container naming configuration (daemon config, not tmux)
- New layouts beyond `lf-dev` and `lf-swarm`

## Implementation plan

### 1. `scripts/lf-up.sh` (~80 lines)

```bash
#!/usr/bin/env bash
# lf-up.sh — bootstrap loopflow: ensure daemon running, open layout

source "$(dirname "$0")/helpers.sh"

main() {
    local detach=false
    [[ "$1" == "--detach" ]] && detach=true

    # Step 1: Check for lfd
    if ! loopflow_has_cmd lfd; then
        echo "loopflow: lfd not found — install: curl -fsSL https://... | sh"
        return 1
    fi

    # Step 2: Check for runtime (docker or podman)
    if ! loopflow_has_cmd docker && ! loopflow_has_cmd podman; then
        echo "loopflow: docker not found — install Docker Desktop: https://docker.com/get-started"
        return 1
    fi

    # Step 3: Start daemon if not running
    local status
    status="$(lfd status --json 2>/dev/null)" || true
    if ! echo "$status" | grep -q '"running"'; then
        _lf_up_msg "starting lfd..."
        lfd start
    fi

    # Step 4: Health gate
    _lf_up_health_gate || return 1

    # Step 5: Open layout (unless --detach)
    if [[ "$detach" == true ]]; then
        _lf_up_msg "ready"
        return 0
    fi

    if _lf_up_in_tmux; then
        "$LOOPFLOW_DIR/scripts/layouts/lf-dev.sh"
    else
        _lf_up_msg "ready (not in tmux — run inside tmux for layout)"
    fi
}

_lf_up_health_gate() {
    local attempts=0 max_attempts=60  # 60 × 250ms = 15s
    while (( attempts < max_attempts )); do
        if lfq status >/dev/null 2>&1; then
            _lf_up_msg "ready"
            return 0
        fi
        (( attempts++ ))
        sleep 0.25
    done
    _lf_up_msg "timeout waiting for lfd health — check: lfd status"
    return 1
}

_lf_up_msg() {
    if _lf_up_in_tmux; then
        tmux display-message "loopflow: $1"
    fi
    echo "loopflow: $1"
}

_lf_up_in_tmux() {
    [[ -n "${TMUX:-}" ]]
}
```

### 2. Dispatch updates in `helpers.sh` (~20 lines changed)

Add `up` case. Update `next` and `land` for container mode:

```bash
next)
    if [[ "$mode" == "container" ]]; then
        local wave
        wave="$(loopflow_pick_wave)" || return 1
        tmux send-keys "lfq next '$wave'" Enter
    else
        # ... existing lf mode code
    fi
    ;;
land)
    if [[ "$mode" == "container" ]]; then
        local wave
        wave="$(loopflow_pick_wave)" || return 1
        tmux send-keys "lfq land '$wave'" Enter
    else
        # ... existing lf mode code
    fi
    ;;
up)
    tmux send-keys "'$LOOPFLOW_DIR/scripts/lf-up.sh'" Enter
    ;;
```

### 3. Keybinding registration (~2 lines)

Add to `scripts/keybindings.sh`:
```bash
tmux bind-key -T loopflow u run-shell "$LOOPFLOW_DIR/scripts/helpers.sh dispatch up"
```

### 4. Status bar health awareness (~15 lines changed in `loopflow-status.sh`)

In the container mode branch, distinguish healthy/starting/offline:
```bash
if lfq status >/dev/null 2>&1; then
    # healthy — show wave count as today
elif lfd status 2>/dev/null | grep -q 'starting\|running'; then
    echo "[lf: starting…]"
else
    echo "[lf: ⚠ offline]"
fi
```

### 5. Help overlay update (~1 line)

Add `prefix+$prefix+u  start/bootstrap` to the help text.

### 6. Test updates in `tmux-review.py` (~20 lines)

- Verify `lf-up.sh` exists and is executable
- Verify `u` keybinding registered in loopflow table
- Verify help overlay contains `u` binding

## Done when

- `scripts/lf-up.sh` bootstraps daemon and opens layout: `./scripts/lf-up.sh` from a tmux session with Docker running starts `lfd`, waits for health, opens `lf-dev`
- `prefix+l+u` triggers bootstrap from a keybinding
- `prefix+l+n` and `prefix+l+d` work in container mode (pick wave, run `lfq next`/`lfq land`)
- Status bar shows `starting…` during daemon startup and `⚠ offline` when daemon is down
- Help overlay shows all 10 keybindings (was 9)
- `tmux-review.py` passes with new structural checks
- All existing tests still pass

Wave success criteria advanced:
- "all keybindings operate (or fail-soft) in both modes" — `next` and `land` now work in container mode
- "`lf up` gives a usable workspace on first run" — bootstrap script with health gate delivers this
- "install + first successful action in <2 minutes" — `lf up` with warm Docker: ~5s
