#!/usr/bin/env bash
# loopflow.tmux — TPM plugin entrypoint
# Sourced by tmux on plugin install/reload.
# Safe to source multiple times (idempotent).

CURRENT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Always set plugin dir (not user-configurable)
tmux set-option -g @loopflow_dir "$CURRENT_DIR"

# Set defaults only if unset
set_default() {
    local existing
    existing="$(tmux show-option -gqv "$1" 2>/dev/null)"
    if [[ -z "$existing" ]]; then
        tmux set-option -g "$1" "$2"
    fi
}

set_default "@loopflow_mode" "auto"
set_default "@loopflow_key_prefix" "l"
set_default "@loopflow_status_ttl_ms" "2000"
set_default "@loopflow_status_timeout_ms" "250"

# Register status interpolation
# #() runs a shell command and interpolates the output
loopflow_status_cmd="#($CURRENT_DIR/scripts/loopflow-status.sh)"

# Append to status-right if not already present
current_status_right="$(tmux show-option -gqv status-right)"
if [[ "$current_status_right" != *"loopflow-status.sh"* ]]; then
    tmux set-option -gq status-right "$loopflow_status_cmd $current_status_right"
fi

# Source keybindings
source "$CURRENT_DIR/scripts/keybindings.sh"
