#!/usr/bin/env bash
# lf-swarm layout: leader + 3 worker panes
#
# +----------------------------------------+
# |   leader pane (orchestration)          |
# +------------+------------+--------------+
# | worker 1   | worker 2   | worker 3     |
# | worktree a | worktree b | worktree c   |
# +------------+------------+--------------+

set -euo pipefail

CURRENT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$CURRENT_DIR/../helpers.sh"

work_dir="${1:-$(loopflow_pane_path)}"
mode="$(loopflow_mode)"

# Determine terminal dimensions
cols="$(tmux display-message -p '#{window_width}' 2>/dev/null || echo 160)"

# Create new window
tmux new-window -n "lf-swarm" -c "$work_dir"

if (( cols < 120 )); then
    # Simplified 2-pane layout for narrow terminals
    tmux split-window -v -c "$work_dir" -p 50

    # Top: monitor
    tmux select-pane -t 0
    if [[ "$mode" == "container" ]] && loopflow_has_cmd lfq; then
        tmux send-keys "watch -n 5 lfq list" Enter
    elif loopflow_has_cmd lf; then
        tmux send-keys "watch -n 5 lf op wt list" Enter
    fi

    # Bottom: worker
    tmux select-pane -t 1
    if loopflow_has_cmd lf; then
        tmux send-keys "# lf op wt create a && cd ../\$(git branch --show-current)-a && lf implement" Enter
    fi
else
    # Full layout: top leader + 3 bottom workers
    # Split top/bottom
    tmux split-window -v -c "$work_dir" -p 50

    # Split bottom into 3 panes
    tmux select-pane -t 1
    tmux split-window -h -c "$work_dir" -p 66
    tmux split-window -h -c "$work_dir" -p 50

    # Leader pane (0): monitor
    tmux select-pane -t 0
    if [[ "$mode" == "container" ]] && loopflow_has_cmd lfq; then
        tmux send-keys "watch -n 5 lfq list" Enter
    elif loopflow_has_cmd lf; then
        tmux send-keys "watch -n 5 lf op wt list" Enter
    fi

    # Worker 1 (1)
    tmux select-pane -t 1
    if loopflow_has_cmd lf; then
        tmux send-keys "# lf op wt create a && cd ../\$(git branch --show-current)-a && lf implement" Enter
    fi

    # Worker 2 (2)
    tmux select-pane -t 2
    if loopflow_has_cmd lf; then
        tmux send-keys "# lf op wt create b && cd ../\$(git branch --show-current)-b && lf implement" Enter
    fi

    # Worker 3 (3)
    tmux select-pane -t 3
    if loopflow_has_cmd lf; then
        tmux send-keys "# lf op wt create c && cd ../\$(git branch --show-current)-c && lf implement" Enter
    fi

    # Focus worker 1
    tmux select-pane -t 1
fi
