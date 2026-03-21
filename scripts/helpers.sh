#!/usr/bin/env bash
# helpers.sh — shared functions for the loopflow tmux plugin
# Sourced by loopflow.tmux and layout/keybinding scripts.

LOOPFLOW_DIR="${LOOPFLOW_DIR:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
LOOPFLOW_CACHE_FILE="/tmp/loopflow-status-${USER}.json"

# ---------------------------------------------------------------------------
# Command checks
# ---------------------------------------------------------------------------

loopflow_has_cmd() {
    command -v "$1" >/dev/null 2>&1
}

# ---------------------------------------------------------------------------
# tmux option helpers
# ---------------------------------------------------------------------------

loopflow_get_option() {
    local option="$1"
    local default="$2"
    local value
    value="$(tmux show-option -gqv "$option" 2>/dev/null)"
    if [[ -z "$value" ]]; then
        echo "$default"
    else
        echo "$value"
    fi
}

loopflow_status_ttl_ms() {
    loopflow_get_option "@loopflow_status_ttl_ms" "2000"
}

loopflow_status_timeout_ms() {
    loopflow_get_option "@loopflow_status_timeout_ms" "250"
}

# ---------------------------------------------------------------------------
# Mode detection
# ---------------------------------------------------------------------------

loopflow_detect_container_mode() {
    if ! loopflow_has_cmd lfq; then
        return 1
    fi
    local timeout_s
    timeout_s="$(awk "BEGIN {printf \"%.1f\", $(loopflow_status_timeout_ms) / 1000}")"
    if loopflow_has_cmd timeout; then
        timeout "$timeout_s" lfq status >/dev/null 2>&1
    else
        lfq status >/dev/null 2>&1
    fi
}

loopflow_mode() {
    local explicit
    explicit="$(loopflow_get_option "@loopflow_mode" "auto")"
    case "$explicit" in
        lf) echo "lf"; return 0 ;;
        container) echo "container"; return 0 ;;
    esac
    # auto detection
    if loopflow_detect_container_mode; then
        echo "container"
    else
        echo "lf"
    fi
}

# ---------------------------------------------------------------------------
# Display
# ---------------------------------------------------------------------------

loopflow_display() {
    tmux display-message "loopflow: $1"
}

# ---------------------------------------------------------------------------
# tmux version helpers
# ---------------------------------------------------------------------------

loopflow_has_popup() {
    local tmux_version major minor
    tmux_version="$(tmux -V 2>/dev/null | sed 's/[^0-9.]//g')"
    major="${tmux_version%%.*}"
    minor="${tmux_version#*.}"
    minor="${minor%%.*}"
    [[ -n "$major" ]] && (( major > 3 || (major == 3 && minor >= 2) ))
}

# ---------------------------------------------------------------------------
# Pane path helper
# ---------------------------------------------------------------------------

loopflow_pane_path() {
    tmux display-message -p -F '#{pane_current_path}' 2>/dev/null || pwd
}

# ---------------------------------------------------------------------------
# Cache helpers
# ---------------------------------------------------------------------------

loopflow_cache_write() {
    local text="$1"
    local mode="$2"
    local source="$3"
    local now
    now="$(date +%s)"
    printf '{"generated_at":%d,"mode":"%s","text":"%s","source":"%s"}\n' \
        "$now" "$mode" "$text" "$source" > "$LOOPFLOW_CACHE_FILE.tmp"
    mv "$LOOPFLOW_CACHE_FILE.tmp" "$LOOPFLOW_CACHE_FILE"
}

loopflow_cache_fresh() {
    if [[ ! -f "$LOOPFLOW_CACHE_FILE" ]]; then
        return 1
    fi
    local ttl_ms now generated_at age_ms
    ttl_ms="$(loopflow_status_ttl_ms)"
    now="$(date +%s)"
    # Parse generated_at from JSON (portable: no jq dependency)
    generated_at="$(sed -n 's/.*"generated_at":\([0-9]*\).*/\1/p' "$LOOPFLOW_CACHE_FILE")"
    if [[ -z "$generated_at" ]]; then
        return 1
    fi
    age_ms=$(( (now - generated_at) * 1000 ))
    if (( age_ms < ttl_ms )); then
        return 0
    fi
    return 1
}

loopflow_cache_text() {
    if [[ ! -f "$LOOPFLOW_CACHE_FILE" ]]; then
        echo ""
        return 1
    fi
    sed -n 's/.*"text":"\([^"]*\)".*/\1/p' "$LOOPFLOW_CACHE_FILE"
}

# ---------------------------------------------------------------------------
# Picker helpers
# ---------------------------------------------------------------------------

# Run fzf, routing through display-popup when no TTY is available (run-shell).
# Args: prompt label, items (newline-separated), result file path
# Returns: 0 if selection made, 1 otherwise. Selection written to result file.
_loopflow_fzf_pick() {
    local prompt="$1" items="$2" result_file="$3"

    if [[ -t 0 ]]; then
        # Direct TTY available — run fzf inline
        local sel
        sel="$(echo "$items" | fzf --prompt="$prompt> " --height=10 --reverse 2>/dev/null)"
        if [[ -n "$sel" ]]; then
            echo "$sel" > "$result_file"
            return 0
        fi
        return 1
    fi

    # No TTY (run-shell context) — use display-popup
    if loopflow_has_popup; then
        local items_file="/tmp/loopflow-pick-items-${USER}.txt"
        echo "$items" > "$items_file"
        tmux display-popup -w 50 -h 12 -E \
            "fzf --prompt='$prompt> ' --height=10 --reverse < '$items_file' > '$result_file'"
        if [[ -s "$result_file" ]]; then
            return 0
        fi
        return 1
    fi

    # No TTY, no popup — caller should use tmux-native fallback
    return 2
}

loopflow_pick_wave() {
    local mode items selection
    mode="$(loopflow_mode)"
    if [[ "$mode" == "container" ]]; then
        if ! loopflow_has_cmd lfq; then
            loopflow_display "lfq not found"
            return 1
        fi
        items="$(lfq list 2>/dev/null | tail -n +2)"
    else
        if loopflow_has_cmd lf; then
            items="$(lf op wt list 2>/dev/null)"
        elif loopflow_has_cmd git; then
            items="$(git worktree list --porcelain 2>/dev/null | grep '^worktree ' | sed 's/^worktree //')"
        fi
    fi

    if [[ -z "$items" ]]; then
        loopflow_display "no waves/worktrees found"
        return 1
    fi

    local result_file="/tmp/loopflow-pick-${USER}.txt"
    rm -f "$result_file"

    if loopflow_has_cmd fzf; then
        _loopflow_fzf_pick "wave" "$items" "$result_file"
        local rc=$?
        if [[ $rc -eq 0 ]]; then
            selection="$(cat "$result_file")"
            rm -f "$result_file"
            echo "$selection"
            return 0
        elif [[ $rc -eq 2 ]]; then
            # No TTY, no popup — use first item as fallback
            selection="$(echo "$items" | head -1)"
            loopflow_display "selected: $selection (fzf needs tmux 3.2+ for popup picker)"
        else
            loopflow_display "nothing selected"
            rm -f "$result_file"
            return 1
        fi
    else
        selection="$(echo "$items" | head -1)"
        loopflow_display "selected: $selection (install fzf for picker)"
    fi

    rm -f "$result_file"
    if [[ -z "$selection" ]]; then
        loopflow_display "nothing selected"
        return 1
    fi
    echo "$selection"
}

loopflow_open_layout() {
    local layouts=("dev" "swarm")
    local selection result_file="/tmp/loopflow-pick-${USER}.txt"
    rm -f "$result_file"

    if loopflow_has_cmd fzf; then
        local items
        items="$(printf '%s\n' "${layouts[@]}")"
        _loopflow_fzf_pick "layout" "$items" "$result_file"
        local rc=$?
        if [[ $rc -eq 0 ]]; then
            selection="$(cat "$result_file")"
            rm -f "$result_file"
        elif [[ $rc -eq 2 ]]; then
            # No TTY, no popup — fall through to display-menu
            selection=""
        else
            loopflow_display "nothing selected"
            rm -f "$result_file"
            return 1
        fi
    fi

    if [[ -z "$selection" ]]; then
        # tmux-native fallback
        tmux display-menu -T "Layout" \
            "dev"   "d" "run-shell '$LOOPFLOW_DIR/scripts/layouts/lf-dev.sh'" \
            "swarm" "s" "run-shell '$LOOPFLOW_DIR/scripts/layouts/lf-swarm.sh'" \
            2>/dev/null
        return 0
    fi

    "$LOOPFLOW_DIR/scripts/layouts/lf-${selection}.sh"
}

# ---------------------------------------------------------------------------
# Command dispatch
# ---------------------------------------------------------------------------

# Pick a wave and send an lfq command to the active pane.
# loopflow_pick_wave already checks for lfq in container mode.
_loopflow_container_wave_cmd() {
    local cmd="$1"
    local wave
    wave="$(loopflow_pick_wave)" || return 1
    tmux send-keys "lfq $cmd '$wave'" Enter
}

loopflow_dispatch() {
    local action="$1"
    local mode
    mode="$(loopflow_mode)"

    case "$action" in
        run)
            if [[ "$mode" == "container" ]]; then
                _loopflow_container_wave_cmd run
            else
                if ! loopflow_has_cmd lf; then
                    loopflow_display "lf not found — install loopflow first"
                    return 1
                fi
                tmux send-keys "lf implement" Enter
            fi
            ;;
        stop)
            if [[ "$mode" == "container" ]]; then
                _loopflow_container_wave_cmd stop
            else
                tmux send-keys C-c
            fi
            ;;
        logs)
            if [[ "$mode" == "container" ]]; then
                _loopflow_container_wave_cmd logs
            else
                loopflow_display "logs: use lf output in terminal"
            fi
            ;;
        pr)
            if loopflow_has_cmd gh; then
                tmux send-keys "gh pr view --web" Enter
            else
                loopflow_display "gh CLI not found"
            fi
            ;;
        next|land)
            if [[ "$mode" == "container" ]]; then
                _loopflow_container_wave_cmd land
            else
                if loopflow_has_cmd lf; then
                    tmux send-keys "lf op $action" Enter
                else
                    loopflow_display "lf not found"
                fi
            fi
            ;;
        wave-pick)
            local wave
            wave="$(loopflow_pick_wave)" || return 1
            if [[ "$mode" == "container" ]]; then
                tmux send-keys "lfq logs '$wave'" Enter
            else
                # Open the worktree in a new lf-dev layout
                "$LOOPFLOW_DIR/scripts/layouts/lf-dev.sh" "$wave"
            fi
            ;;
        layout-pick)
            loopflow_open_layout
            ;;
        up)
            tmux send-keys "'$LOOPFLOW_DIR/scripts/lfd-up.sh'" Enter
            ;;
        help)
            loopflow_show_help
            ;;
        *)
            loopflow_display "unknown action: $action"
            return 1
            ;;
    esac
}

# ---------------------------------------------------------------------------
# Help overlay
# ---------------------------------------------------------------------------

loopflow_show_help() {
    local prefix
    prefix="$(loopflow_get_option "@loopflow_key_prefix" "l")"
    local mode
    mode="$(loopflow_mode)"

    local help_file="/tmp/loopflow-help-${USER}.txt"
    cat > "$help_file" <<EOF
loopflow keybindings (mode: $mode)
─────────────────────────────
prefix+$prefix+r  run step/wave
prefix+$prefix+s  stop
prefix+$prefix+o  open logs
prefix+$prefix+p  open PR
prefix+$prefix+n  next iteration
prefix+$prefix+d  land PR
prefix+$prefix+u  start/bootstrap
prefix+$prefix+w  pick wave
prefix+$prefix+L  pick layout
prefix+$prefix+?  this help
EOF

    # Try display-popup (tmux 3.2+), fallback to display-message
    if loopflow_has_popup; then
        tmux display-popup -w 40 -h 15 -E "cat '$help_file'; read -n 1"
    else
        loopflow_display "prefix+$prefix+{r,s,o,p,n,d,u,w,L,?} — use ? in popup-capable tmux 3.2+"
    fi
}
