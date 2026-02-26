#!/usr/bin/env bash
# keybindings.sh — register loopflow keybindings in tmux
# Sourced by loopflow.tmux on plugin load.
# Idempotent: unbinds before rebinding.

CURRENT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
LOOPFLOW_DIR="$(cd "$CURRENT_DIR/.." && pwd)"

# Read key prefix (default: l)
key_prefix="$(tmux show-option -gqv @loopflow_key_prefix 2>/dev/null)"
key_prefix="${key_prefix:-l}"

helpers="$CURRENT_DIR/helpers.sh"

# ---------------------------------------------------------------------------
# Binding helper
# ---------------------------------------------------------------------------

bind_loopflow_key() {
    local key="$1"
    local action="$2"

    # Unbind first for idempotency
    tmux unbind-key -T loopflow "$key" 2>/dev/null

    # Bind: prefix -> key_prefix enters loopflow table, then key dispatches action
    tmux bind-key -T loopflow "$key" \
        run-shell "source '$helpers' && loopflow_dispatch '$action'"
}

# ---------------------------------------------------------------------------
# Register the prefix key to enter the loopflow key table
# ---------------------------------------------------------------------------

# Unbind old prefix binding for idempotency
tmux unbind-key -T prefix "$key_prefix" 2>/dev/null

# prefix + key_prefix switches to the loopflow key table
tmux bind-key -T prefix "$key_prefix" switch-client -T loopflow

# ---------------------------------------------------------------------------
# Register action bindings
# ---------------------------------------------------------------------------

bind_loopflow_key "r" "run"
bind_loopflow_key "s" "stop"
bind_loopflow_key "o" "logs"
bind_loopflow_key "p" "pr"
bind_loopflow_key "n" "next"
bind_loopflow_key "d" "land"
bind_loopflow_key "w" "wave-pick"
bind_loopflow_key "L" "layout-pick"
bind_loopflow_key "?" "help"
