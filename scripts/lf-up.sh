#!/usr/bin/env bash
# lf-up.sh — bootstrap loopflow: ensure daemon running, open layout
#
# Usage:
#   scripts/lf-up.sh [--detach]
#
# Starts lfd if not running, waits for health, then opens lf-dev layout.
# Works outside tmux too (skips layout, just starts daemon).

CURRENT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
LOOPFLOW_DIR="$(cd "$CURRENT_DIR/.." && pwd)"
source "$CURRENT_DIR/helpers.sh"

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

_lf_up_in_tmux() {
    [[ -n "${TMUX:-}" ]]
}

_lf_up_msg() {
    if _lf_up_in_tmux; then
        tmux display-message "loopflow: $1"
    fi
    echo "loopflow: $1"
}

_lf_up_health_gate() {
    local attempts=0 max_attempts=60  # 60 * 250ms = 15s
    while (( attempts < max_attempts )); do
        if lfq status >/dev/null 2>&1; then
            _lf_up_msg "ready"
            return 0
        fi
        (( attempts++ ))
        sleep 0.25
    done
    _lf_up_msg "timeout waiting for lfd health -- check: lfd status"
    return 1
}

# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------

main() {
    local detach=false
    [[ "$1" == "--detach" ]] && detach=true

    # Step 1: Check for lfd
    if ! loopflow_has_cmd lfd; then
        echo "loopflow: lfd not found -- install: curl -fsSL https://github.com/loopflowstudio/loopflow/releases/latest/download/install.sh | sh"
        return 1
    fi

    # Step 2: Check for container runtime (docker or podman)
    if ! loopflow_has_cmd docker && ! loopflow_has_cmd podman; then
        echo "loopflow: container runtime not found -- install Docker Desktop: https://docker.com/get-started"
        return 1
    fi

    # Step 3: Start daemon if not running
    if ! lfq status >/dev/null 2>&1; then
        _lf_up_msg "starting lfd..."
        lfd start
    fi

    # Step 4: Health gate — poll until daemon is healthy
    _lf_up_health_gate || return 1

    # Step 5: Open layout (unless --detach)
    if [[ "$detach" == true ]]; then
        return 0
    fi

    if _lf_up_in_tmux; then
        # Idempotent: focus existing lf-dev window or create new one
        if tmux list-windows -F '#{window_name}' 2>/dev/null | grep -q '^lf-dev$'; then
            tmux select-window -t lf-dev
        else
            "$LOOPFLOW_DIR/scripts/layouts/lf-dev.sh"
        fi
    else
        _lf_up_msg "ready (not in tmux -- run inside tmux for layout)"
    fi
}

main "$@"
