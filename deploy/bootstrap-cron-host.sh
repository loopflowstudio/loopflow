#!/usr/bin/env bash
# Bootstrap this checkout as a maintained self-hosted loopflow cron host.
# Secrets come from Doppler when configured. Use LOOPFLOW_SECRETS=env to force
# plain environment variables.

set -euo pipefail

usage() {
    cat >&2 <<'USAGE'
Usage: bootstrap-cron-host.sh [--repo PATH] [--host auto|linux|mac] [--no-service] [--no-wave]

Installs host service wiring, starts the self-hosted lfd stack, waits for
health, and creates/shows the root wave on the remote daemon.

Environment:
  LOOPFLOW_SECRETS=auto|doppler|env   default: auto
  LF_DOMAIN=lfd.example.com           required host name for remote clients
  LF_TLS_MODE=internal                private/Tailscale TLS; leave empty for public ACME
  LFD_AUTH_TOKEN=...                  required bearer token for remote lfd
  LFD_URL=https://lfd.example.com     optional override for wave setup
USAGE
}

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo="$(cd "$script_dir/.." && pwd)"
host="auto"
install_service=1
create_wave=1

while [[ $# -gt 0 ]]; do
    case "$1" in
        --repo)
            if [[ $# -lt 2 ]]; then
                echo "--repo requires a path" >&2
                usage
                exit 1
            fi
            repo="$2"
            shift 2
            ;;
        --repo=*)
            repo="${1#--repo=}"
            shift
            ;;
        --host)
            if [[ $# -lt 2 ]]; then
                echo "--host requires linux, mac, or auto" >&2
                usage
                exit 1
            fi
            host="$2"
            shift 2
            ;;
        --host=*)
            host="${1#--host=}"
            shift
            ;;
        --no-service)
            install_service=0
            shift
            ;;
        --no-wave)
            create_wave=0
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

repo="$(git -C "$repo" rev-parse --show-toplevel)"

case "$host" in
    auto)
        case "$(uname -s)" in
            Darwin) host="mac" ;;
            Linux) host="linux" ;;
            *) echo "cannot infer host type; pass --host linux or --host mac" >&2; exit 1 ;;
        esac
        ;;
    linux|mac)
        ;;
    *)
        echo "invalid --host: $host" >&2
        usage
        exit 1
        ;;
esac

has_doppler_config() {
    command -v doppler >/dev/null 2>&1 || return 1
    [[ -n "${DOPPLER_TOKEN:-}" ]] && return 0
    (cd "$repo" && doppler configure get project --plain >/dev/null 2>&1)
}

run_with_secrets() {
    case "${LOOPFLOW_SECRETS:-auto}" in
        env)
            "$@"
            ;;
        doppler)
            command -v doppler >/dev/null 2>&1 || {
                echo "LOOPFLOW_SECRETS=doppler but doppler is not installed" >&2
                exit 1
            }
            (cd "$repo" && doppler run -- "$@")
            ;;
        auto)
            if has_doppler_config; then
                (cd "$repo" && doppler run -- "$@")
            else
                "$@"
            fi
            ;;
        *)
            echo "invalid LOOPFLOW_SECRETS: ${LOOPFLOW_SECRETS}" >&2
            exit 1
            ;;
    esac
}

preflight_bootstrap_env() {
    run_with_secrets bash -c '
        missing=0
        for name in LF_DOMAIN LFD_AUTH_TOKEN; do
            if [ -z "${!name:-}" ]; then
                echo "$name is required for cron-host bootstrap" >&2
                missing=1
            fi
        done
        exit "$missing"
    '
}

install_mac_launch_agent() {
    local dest="$HOME/Library/LaunchAgents/loopflow.server.plist"
    mkdir -p "$(dirname "$dest")"
    cp "$repo/deploy/launchd/loopflow.server.plist" "$dest"
    python3 - "$dest" "$repo/deploy/loopflow-server.sh" <<'PY_PLIST'
import os
import plistlib
import sys
from pathlib import Path

path = Path(sys.argv[1])
server_script = sys.argv[2]
with path.open("rb") as handle:
    data = plistlib.load(handle)

data["ProgramArguments"] = [server_script, "up"]

env = data.setdefault("EnvironmentVariables", {})
env.setdefault(
    "PATH",
    os.environ.get(
        "PATH",
        "/opt/homebrew/bin:/opt/homebrew/sbin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin",
    ),
)
for name in ["DOPPLER_TOKEN", "DOCKER_CONFIG", "DOCKER_HOST", "LOOPFLOW_SECRETS", "LF_DOMAIN", "LF_TLS_MODE", "LFD_AUTH_TOKEN", "LFD_PORT"]:
    value = os.environ.get(name)
    if value:
        env[name] = value

with path.open("wb") as handle:
    plistlib.dump(data, handle)
PY_PLIST
    launchctl bootout "gui/$(id -u)" "$dest" >/dev/null 2>&1 || true
    launchctl bootstrap "gui/$(id -u)" "$dest"
    launchctl enable "gui/$(id -u)/loopflow.server"
}

write_linux_env_file() {
    local sudo_cmd=("$@")
    local tmp
    tmp="$(mktemp)"
    {
        printf 'LOOPFLOW_SECRETS=%q\n' "${LOOPFLOW_SECRETS:-auto}"
        for name in DOPPLER_TOKEN LF_DOMAIN LF_TLS_MODE LFD_AUTH_TOKEN LFD_PORT; do
            if [[ -n "${!name:-}" ]]; then
                printf '%s=%q\n' "$name" "${!name}"
            fi
        done
    } > "$tmp"
    "${sudo_cmd[@]}" install -m 0600 "$tmp" /etc/loopflow-server.env
    rm -f "$tmp"
}

install_linux_systemd_units() {
    local sudo_cmd=()
    if [[ "$(id -u)" -ne 0 ]]; then
        sudo_cmd=(sudo)
    fi

    write_linux_env_file "${sudo_cmd[@]}"

    for unit in loopflow-server.service loopflow-server-update.service loopflow-server-update.timer; do
        local src="$repo/deploy/systemd/$unit"
        local tmp
        tmp="$(mktemp)"
        sed "s#/opt/loopflow#$repo#g" "$src" > "$tmp"
        "${sudo_cmd[@]}" install -m 0644 "$tmp" "/etc/systemd/system/$unit"
        rm -f "$tmp"
    done

    "${sudo_cmd[@]}" systemctl daemon-reload
    "${sudo_cmd[@]}" systemctl enable --now loopflow-server.service
    "${sudo_cmd[@]}" systemctl enable --now loopflow-server-update.timer
}

wait_for_health() {
    local attempts=30
    until "$repo/deploy/loopflow-server.sh" --repo "$repo" health >/dev/null 2>&1; do
        attempts=$((attempts - 1))
        if [[ "$attempts" -le 0 ]]; then
            echo "lfd did not become healthy" >&2
            echo "Run: $repo/deploy/loopflow-server.sh logs" >&2
            exit 1
        fi
        sleep 2
    done
}

create_root_wave() {
    # A wave is its markdown: wave/root/GOAL.md in the repo. lfd derives the
    # rest at boot; nothing to POST.
    local wave_dir="$repo/wave/root"
    if [[ -f "$wave_dir/GOAL.md" ]]; then
        echo "root wave already authored at $wave_dir/GOAL.md"
        return 0
    fi
    mkdir -p "$wave_dir"
    cat > "$wave_dir/GOAL.md" <<'GOAL'
Drive this repo's roadmap.
GOAL
    : > "$wave_dir/MEMORY.md"
    echo "authored root wave at $wave_dir/GOAL.md"
}

preflight_bootstrap_env

if [[ "$install_service" -eq 1 ]]; then
    case "$host" in
        mac) install_mac_launch_agent ;;
        linux) install_linux_systemd_units ;;
    esac
else
    "$repo/deploy/loopflow-server.sh" --repo "$repo" up
fi

wait_for_health

if [[ "$create_wave" -eq 1 ]]; then
    create_root_wave
fi

run_with_secrets bash -c 'echo "loopflow cron host ready: ${LFD_URL:-https://${LF_DOMAIN}}"'
