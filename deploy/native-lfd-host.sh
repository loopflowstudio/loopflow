#!/usr/bin/env bash
# Manage a native macOS self-hosted lfd service.
# Use this for private Mac/Tailscale hosts where Docker Compose is not the desired runtime.

set -euo pipefail

usage() {
    cat >&2 <<'USAGE'
Usage: native-lfd-host.sh [--repo PATH] [--install-dir PATH] <install|install-update-agent|update|restart|status|logs|health>

Commands:
  install              Build/install lf+lfd, install launchd service/update agent, and wait for health
  install-update-agent Install the nightly launchd update agent
  update               Pull default branch, rebuild/install lf+lfd, restart service, and wait for health
  restart              Restart the launchd lfd service and wait for health
  status               Show launchd service/update-agent state and local daemon status
  logs                 Tail native lfd launchd logs
  health               Check local daemon health and authenticated status when token is available

Environment:
  LFD_HTTP_ADDR=0.0.0.0:2486      listen address for remote/private clients
  LFD_AUTH_TOKEN=...              required for install/update/restart on non-loopback hosts
  LFD_AUTH_TOKEN_FILE=~/.lf/lfd-token token file used when LFD_AUTH_TOKEN is unset
  LFD_PORT=2486                   health-check port derived from LFD_HTTP_ADDR when unset
USAGE
}

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo="$(cd "$script_dir/.." && pwd)"
install_dir="${LF_INSTALL_DIR:-$HOME/.local/bin}"
command=""
label="com.loopflow.lfd"
update_label="com.loopflow.lfd.update"
plist="$HOME/Library/LaunchAgents/$label.plist"
update_plist="$HOME/Library/LaunchAgents/$update_label.plist"
token_file="${LFD_AUTH_TOKEN_FILE:-$HOME/.lf/lfd-token}"

while [[ $# -gt 0 ]]; do
    case "$1" in
        --repo)
            repo="$2"
            shift 2
            ;;
        --repo=*)
            repo="${1#--repo=}"
            shift
            ;;
        --install-dir)
            install_dir="$2"
            shift 2
            ;;
        --install-dir=*)
            install_dir="${1#--install-dir=}"
            shift
            ;;
        install|install-update-agent|update|restart|status|logs|health)
            command="$1"
            shift
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            echo "unknown argument: $1" >&2
            usage
            exit 1
            ;;
    esac
done

if [[ -z "$command" ]]; then
    usage
    exit 1
fi

repo="$(git -C "$repo" rev-parse --show-toplevel)"
install_dir="$(mkdir -p "$install_dir" && cd "$install_dir" && pwd)"
lfd_bin="$install_dir/lfd"
script_path="$repo/deploy/native-lfd-host.sh"
http_addr="${LFD_HTTP_ADDR:-0.0.0.0:${LFD_PORT:-2486}}"
port="${LFD_PORT:-${http_addr##*:}}"
log_dir="$HOME/.lf/logs"
log_out="$log_dir/lfd.log"
update_log="$log_dir/lfd-update.log"

load_token() {
    if [[ -z "${LFD_AUTH_TOKEN:-}" && -r "$token_file" ]]; then
        LFD_AUTH_TOKEN="$(tr -d '\r\n' < "$token_file")"
        export LFD_AUTH_TOKEN
    fi
}

require_token() {
    load_token
    if [[ -z "${LFD_AUTH_TOKEN:-}" ]]; then
        echo "LFD_AUTH_TOKEN or readable LFD_AUTH_TOKEN_FILE is required for native private lfd host management" >&2
        exit 1
    fi
}

ensure_token_file() {
    require_token
    if [[ ! -r "$token_file" ]]; then
        mkdir -p "$(dirname "$token_file")"
        chmod 700 "$(dirname "$token_file")" 2>/dev/null || true
        umask 077
        printf '%s\n' "$LFD_AUTH_TOKEN" > "$token_file"
        chmod 0600 "$token_file"
    fi
}

service_target() {
    echo "gui/$(id -u)/$label"
}

update_target() {
    echo "gui/$(id -u)/$update_label"
}

install_bins() {
    "$repo/scripts/pull-local-bin.sh" --repo "$repo" --install-dir "$install_dir" "$@"
}

wait_for_health() {
    local attempts=60
    until curl -fsS "http://127.0.0.1:$port/health" >/dev/null 2>&1; do
        attempts=$((attempts - 1))
        if [[ "$attempts" -le 0 ]]; then
            echo "native lfd did not become healthy" >&2
            echo "Run: $0 logs" >&2
            exit 1
        fi
        sleep 1
    done
}

install_service() {
    require_token
    if [[ ! -x "$lfd_bin" ]]; then
        echo "missing lfd binary: $lfd_bin" >&2
        exit 1
    fi
    LFD_HTTP_ADDR="$http_addr" LFD_AUTH_TOKEN="$LFD_AUTH_TOKEN" "$lfd_bin" install --force
}

install_update_agent() {
    ensure_token_file
    mkdir -p "$(dirname "$update_plist")" "$log_dir"
    chmod 700 "$HOME/.lf" "$log_dir" 2>/dev/null || true
    python3 - "$update_plist" "$script_path" "$repo" "$install_dir" "$http_addr" "$token_file" "$update_log" <<'PY'
import os
import plistlib
import sys
from pathlib import Path

plist_path, script_path, repo, install_dir, http_addr, token_file, log_path = sys.argv[1:]
data = {
    "Label": "com.loopflow.lfd.update",
    "ProgramArguments": [
        script_path,
        "--repo",
        repo,
        "--install-dir",
        install_dir,
        "update",
    ],
    "StartCalendarInterval": {
        "Hour": 4,
        "Minute": 30,
    },
    "StandardOutPath": log_path,
    "StandardErrorPath": log_path,
    "EnvironmentVariables": {
        "PATH": os.environ.get("PATH", ""),
        "LFD_HTTP_ADDR": http_addr,
        "LFD_AUTH_TOKEN_FILE": token_file,
    },
}
Path(plist_path).write_bytes(plistlib.dumps(data))
PY
    chmod 0600 "$update_plist"
    launchctl bootout "$(update_target)" >/dev/null 2>&1 || true
    launchctl bootstrap "gui/$(id -u)" "$update_plist"
    echo "installed: $update_plist"
}

restart_service() {
    require_token
    launchctl kickstart -k "$(service_target)" >/dev/null 2>&1 || {
        echo "failed to restart $label; run install first" >&2
        exit 1
    }
    wait_for_health
}

health() {
    curl -fsS "http://127.0.0.1:$port/health"
    echo
    load_token
    if [[ -n "${LFD_AUTH_TOKEN:-}" ]]; then
        curl -fsS -H "Authorization: Bearer $LFD_AUTH_TOKEN" "http://127.0.0.1:$port/status"
        echo
    fi
}

case "$command" in
    install)
        install_bins
        install_service
        install_update_agent
        wait_for_health
        health
        ;;
    install-update-agent)
        install_update_agent
        ;;
    update)
        install_bins
        restart_service
        health
        ;;
    restart)
        restart_service
        health
        ;;
    status)
        launchctl print "$(service_target)" 2>/dev/null || true
        launchctl print "$(update_target)" 2>/dev/null || true
        health || true
        ;;
    logs)
        tail -f "$log_out" "$update_log"
        ;;
    health)
        health
        ;;
esac
