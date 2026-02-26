#!/usr/bin/env bash
# loopflow-status.sh — status bar renderer for tmux
# Called by tmux status-interval via #().
# Must complete quickly (<100ms hot path, <250ms cold path).

CURRENT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$CURRENT_DIR/helpers.sh"

# ---------------------------------------------------------------------------
# Status generation
# ---------------------------------------------------------------------------

generate_lf_status() {
    local pane_path branch status_text

    # Get git branch from pane's cwd
    pane_path="$(loopflow_pane_path)"
    branch="$(git -C "$pane_path" rev-parse --abbrev-ref HEAD 2>/dev/null)"

    if [[ -z "$branch" ]]; then
        echo "[lf]"
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

    if [[ -n "$active_step" ]]; then
        status_text="$branch ▶ $active_step"
    else
        status_text="$branch"
    fi

    echo "[lf: $status_text]"
}

generate_container_status() {
    if ! loopflow_has_cmd lfq; then
        echo "[lf: --]"
        return
    fi

    local timeout_ms output
    timeout_ms="$(loopflow_status_timeout_ms)"

    # Get wave list from lfq
    local timeout_s
    timeout_s="$(awk "BEGIN {printf \"%.1f\", $timeout_ms / 1000}")"

    if loopflow_has_cmd timeout; then
        output="$(timeout "$timeout_s" lfq list --json 2>/dev/null)"
    else
        output="$(lfq list --json 2>/dev/null)"
    fi

    if [[ -z "$output" ]] || [[ "$output" == "null" ]]; then
        echo "[lf: idle]"
        return
    fi

    # Parse wave count and active wave (portable JSON parsing)
    local wave_count active_wave
    wave_count="$(echo "$output" | grep -c '"name"' 2>/dev/null || echo "0")"
    active_wave="$(echo "$output" | grep -o '"name":"[^"]*"' | head -1 | sed 's/"name":"//;s/"//' 2>/dev/null)"

    if [[ "$wave_count" -eq 0 ]]; then
        echo "[lf: idle]"
    elif [[ -n "$active_wave" ]]; then
        echo "[lf: $wave_count waves | $active_wave]"
    else
        echo "[lf: $wave_count waves]"
    fi
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

    local mode status_text source="live"

    mode="$(loopflow_mode)"

    case "$mode" in
        container)
            status_text="$(generate_container_status)"
            ;;
        *)
            status_text="$(generate_lf_status)"
            ;;
    esac

    # If generation failed, try stale cache
    if [[ -z "$status_text" ]]; then
        local cached
        cached="$(loopflow_cache_text)"
        if [[ -n "$cached" ]]; then
            status_text="${cached}~"
            source="cache"
        else
            status_text="[lf]"
            source="fallback"
        fi
    fi

    # Write cache for next invocation
    loopflow_cache_write "$status_text" "$mode" "$source"

    echo "$status_text"
}

main "$@"
