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

generate_container_status() {
    if ! loopflow_has_cmd lfq; then
        loopflow_apply_format "--" "" "" "" ""
        return
    fi

    local timeout_ms
    timeout_ms="$(loopflow_status_timeout_ms)"
    local timeout_s
    timeout_s="$(awk "BEGIN {printf \"%.1f\", $timeout_ms / 1000}")"

    # Check daemon health first
    local lfq_ok=false
    if loopflow_has_cmd timeout; then
        timeout "$timeout_s" lfq status >/dev/null 2>&1 && lfq_ok=true
    else
        lfq status >/dev/null 2>&1 && lfq_ok=true
    fi

    if [[ "$lfq_ok" != true ]]; then
        # Daemon not responding — check if it's starting or offline
        local lfd_out=""
        if loopflow_has_cmd lfd; then
            if loopflow_has_cmd timeout; then
                lfd_out="$(timeout "$timeout_s" lfd status 2>/dev/null)"
            else
                lfd_out="$(lfd status 2>/dev/null)"
            fi
        fi
        if echo "$lfd_out" | grep -qiE 'starting|running'; then
            loopflow_apply_format "starting..." "" "" "" ""
        else
            loopflow_apply_format "! offline" "" "" "" ""
        fi
        return
    fi

    # Daemon healthy — get wave list
    local output
    if loopflow_has_cmd timeout; then
        output="$(timeout "$timeout_s" lfq list --json 2>/dev/null)"
    else
        output="$(lfq list --json 2>/dev/null)"
    fi

    if [[ -z "$output" ]] || [[ "$output" == "null" ]]; then
        loopflow_apply_format "idle" "" "" "0" ""
        return
    fi

    # Parse wave count and active wave (portable JSON — assumes top-level array of objects)
    local wave_count active_wave status_text
    wave_count="$(echo "$output" | grep -c '"name"\s*:' 2>/dev/null || echo "0")"
    active_wave="$(echo "$output" | grep -o '"name":"[^"]*"' | head -1 | sed 's/"name":"//;s/"//' 2>/dev/null)"

    if [[ "$wave_count" -eq 0 ]]; then
        loopflow_apply_format "idle" "" "" "0" ""
    elif [[ -n "$active_wave" ]]; then
        status_text="$wave_count waves | $active_wave"
        loopflow_apply_format "$status_text" "" "" "$wave_count" "$active_wave"
    else
        status_text="$wave_count waves"
        loopflow_apply_format "$status_text" "" "" "$wave_count" ""
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
            status_text="$(loopflow_apply_format "" "" "" "" "")"
            source="fallback"
        fi
    fi

    # Write cache for next invocation
    loopflow_cache_write "$status_text" "$mode" "$source"

    echo "$status_text"
}

main "$@"
