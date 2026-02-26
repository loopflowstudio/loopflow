#!/usr/bin/env bash
# lf-flow layout: status + flow output + shell
#
# +------------------+----------------------+
# |  lfq list        |   flow output        |
# |  (refreshing)    |   (lf flow build)    |
# |                  +----------------------+
# |                  |   lazygit / shell    |
# +------------------+----------------------+

set -euo pipefail

CURRENT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$CURRENT_DIR/../helpers.sh"

work_dir="${1:-$(loopflow_pane_path)}"
mode="$(loopflow_mode)"

# Determine terminal dimensions
cols="$(tmux display-message -p '#{window_width}' 2>/dev/null || echo 160)"

# Create new window
tmux new-window -n "lf-flow" -c "$work_dir"

if (( cols < 120 )); then
    # Simplified 2-pane layout for narrow terminals
    tmux split-window -v -c "$work_dir" -p 40

    tmux select-pane -t 0
    if [[ "$mode" == "container" ]] && loopflow_has_cmd lfq; then
        tmux send-keys "lfq list" Enter
    elif loopflow_has_cmd lf; then
        tmux send-keys "# lf flow build  (start flow here)" Enter
    fi

    # Bottom: shell
    tmux select-pane -t 1
else
    # Full 3-pane layout
    # Split left (status) | right
    tmux split-window -h -c "$work_dir" -p 65

    # Split right: top (flow output) | bottom (shell/lazygit)
    tmux split-window -v -c "$work_dir" -p 35

    # Left pane (0): status/list
    tmux select-pane -t 0
    if [[ "$mode" == "container" ]] && loopflow_has_cmd lfq; then
        tmux send-keys "watch -n 5 lfq list" Enter
    elif loopflow_has_cmd lf; then
        tmux send-keys "git log --oneline -20" Enter
    fi

    # Top-right pane (1): flow output
    tmux select-pane -t 1
    if [[ "$mode" == "container" ]] && loopflow_has_cmd lfq; then
        tmux send-keys "# lfq run <wave>  (start flow here)" Enter
    elif loopflow_has_cmd lf; then
        tmux send-keys "# lf flow build" Enter
    fi

    # Bottom-right pane (2): lazygit or shell
    tmux select-pane -t 2
    if loopflow_has_cmd lazygit; then
        tmux send-keys "lazygit" Enter
    fi
fi
