#!/usr/bin/env bash
# lf-dev layout: editor + agent + shell
#
# +------------------+----------------------+
# |                  |                      |
# |   editor         |   lf <step>          |
# |   ($EDITOR)      |   (agent output)     |
# |                  +----------------------+
# |                  |   shell              |
# +------------------+----------------------+

set -euo pipefail

CURRENT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$CURRENT_DIR/../helpers.sh"

work_dir="${1:-$(loopflow_pane_path)}"
mode="$(loopflow_mode)"

# Determine terminal dimensions
cols="$(tmux display-message -p '#{window_width}' 2>/dev/null || echo 160)"

# Create new window
tmux new-window -n "lf-dev" -c "$work_dir"

if (( cols < 120 )); then
    # Simplified 2-pane layout for narrow terminals
    tmux split-window -v -c "$work_dir" -p 40

    # Top pane: agent
    tmux select-pane -t 0
    if [[ "$mode" == "container" ]] && loopflow_has_cmd lfq; then
        tmux send-keys "lfq logs" Enter
    elif loopflow_has_cmd lf; then
        tmux send-keys "# lf implement  (run your step here)" Enter
    fi

    # Bottom pane: shell (already there)
    tmux select-pane -t 1
else
    # Full 3-pane layout
    # Split vertically: left (editor) | right
    tmux split-window -h -c "$work_dir" -p 55

    # Split right pane horizontally: top (agent) | bottom (shell)
    tmux split-window -v -c "$work_dir" -p 35

    # Left pane (0): editor
    tmux select-pane -t 0
    if [[ -n "${EDITOR:-}" ]]; then
        tmux send-keys "$EDITOR ." Enter
    else
        tmux send-keys "# set \$EDITOR to auto-open your editor here" Enter
    fi

    # Top-right pane (1): agent
    tmux select-pane -t 1
    if [[ "$mode" == "container" ]] && loopflow_has_cmd lfq; then
        tmux send-keys "# lfq run <wave>  (ride a wave)" Enter
    elif loopflow_has_cmd lf; then
        tmux send-keys "# lf implement  (run your step here)" Enter
    else
        tmux send-keys "# install loopflow: curl -fsSL https://github.com/loopflowstudio/loopflow/releases/latest/download/install.sh | sh" Enter
    fi

    # Bottom-right pane (2): shell (focus here)
    tmux select-pane -t 2
fi
