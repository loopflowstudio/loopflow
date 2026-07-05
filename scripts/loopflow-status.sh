#!/usr/bin/env bash
# loopflow-status.sh — status bar renderer for tmux
# Called by tmux status-interval via #().
# Must complete quickly (<100ms hot path, <250ms cold path).

CURRENT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$CURRENT_DIR/helpers.sh"

# ---------------------------------------------------------------------------
# Format application
# ---------------------------------------------------------------------------

# Apply @loopflow_status_format template with variable substitution.
# Variables: #{status} (computed text), #{branch}, #{step}, #{waves}, #{wave}
loopflow_apply_format() {
    local status="$1" branch="$2" step="$3" waves="$4" wave="$5"
    local fmt
    fmt="$(loopflow_get_option "@loopflow_status_format" "[lf: #{status}]")"
    fmt="${fmt//\#\{status\}/$status}"
    fmt="${fmt//\#\{branch\}/$branch}"
    fmt="${fmt//\#\{step\}/$step}"
    fmt="${fmt//\#\{waves\}/$waves}"
    fmt="${fmt//\#\{wave\}/$wave}"
    echo "$fmt"
}

# ---------------------------------------------------------------------------
# Status generation
# ---------------------------------------------------------------------------

generate_lf_status() {
    local pane_path branch

    # Get git branch from pane's cwd
    pane_path="$(loopflow_pane_path)"
    branch="$(git -C "$pane_path" rev-parse --abbrev-ref HEAD 2>/dev/null)"

    if [[ -z "$branch" ]]; then
        loopflow_apply_format "" "" "" "" ""
        return
    fi

    # Check for active lf process
    local active_step=""
    if loopflow_has_cmd pgrep; then
        local lf_proc
        lf_proc="$(pgrep -af "lf (implement|design|review|debug|gate|compress|research)" 2>/dev/null | head -1)"
        if [[ -n "$lf_proc" ]]; then
            # Extract step name from process command
            active_step="$(echo "$lf_proc" | grep -oE '(implement|design|review|debug|gate|compress|research)' | head -1)"
        fi
    fi

    local status_text
    if [[ -n "$active_step" ]]; then
        status_text="$branch ▶ $active_step"
    else
        status_text="$branch"
    fi

    loopflow_apply_format "$status_text" "$branch" "$active_step" "" ""
}

# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------

main() {
    # Hot path: return cached result if fresh
    if loopflow_cache_fresh; then
        loopflow_cache_text
        return 0
    fi

    local status_text source="live"

    status_text="$(generate_lf_status)"

    # If generation failed, try stale cache
    if [[ -z "$status_text" ]]; then
        local cached
        cached="$(loopflow_cache_text)"
        if [[ -n "$cached" ]]; then
            status_text="${cached}~"
            source="cache"
        else
            status_text="$(loopflow_apply_format "" "" "" "" "")"
            source="fallback"
        fi
    fi

    # Write cache for next invocation
    loopflow_cache_write "$status_text" "lf" "$source"

    echo "$status_text"
}

main "$@"
