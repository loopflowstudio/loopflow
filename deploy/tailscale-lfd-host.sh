#!/usr/bin/env bash
# Manage a Tailscale-fronted native lfd host.
#
# Thin wrapper over native-lfd-host.sh: lfd binds loopback and `tailscale serve`
# terminates HTTPS in front of it, so clients reach lfd at
#   https://<host>.<tailnet>.ts.net
# with a real cert and never over plain http on the tailnet.
#
# Prerequisite: enable "HTTPS Certificates" for the tailnet once in the admin
# console (https://login.tailscale.com/admin/dns).

set -euo pipefail

usage() {
    cat >&2 <<'USAGE'
Usage: tailscale-lfd-host.sh [--repo PATH] [--install-dir PATH] <command>

Commands (forwarded to native-lfd-host.sh unless noted):
  install              Install lfd (bound to loopback) + start the HTTPS front
  install-update-agent Install the nightly update agent
  update               Pull, rebuild, restart lfd
  restart              Restart lfd
  status               Show lfd + `tailscale serve` status
  logs                 Tail lfd logs
  health               Check local lfd health
  serve-off            Tear down the HTTPS front (leaves lfd running)

Environment:
  LFD_PORT=2486         loopback port lfd listens on (the front proxies to it)
  TS_HTTPS_PORT=443     HTTPS port the tailnet front listens on
  LFD_AUTH_TOKEN[_FILE] passed through to native-lfd-host.sh
USAGE
}

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
native="$script_dir/native-lfd-host.sh"

port="${LFD_PORT:-2486}"
https_port="${TS_HTTPS_PORT:-443}"

# lfd must not be exposed directly on the tailnet — the HTTPS front is the only
# ingress. Force loopback before forwarding to native-lfd-host.sh, which bakes
# LFD_HTTP_ADDR into the launchd plist.
export LFD_HTTP_ADDR="127.0.0.1:${port}"

resolve_tailscale() {
    if command -v tailscale >/dev/null 2>&1; then
        command -v tailscale
        return
    fi
    local candidate
    for candidate in /usr/local/bin/tailscale \
        /Applications/Tailscale.app/Contents/MacOS/Tailscale; do
        if [[ -x "$candidate" ]]; then
            echo "$candidate"
            return
        fi
    done
    echo "tailscale CLI not found (install Tailscale)" >&2
    exit 1
}

magic_dns_name() {
    "$ts" status --json 2>/dev/null \
        | python3 -c 'import sys, json; print(json.load(sys.stdin)["Self"]["DNSName"].rstrip("."))' \
        2>/dev/null || true
}

serve_on() {
    "$ts" serve --bg --https="$https_port" "http://127.0.0.1:${port}"
    local name suffix=""
    name="$(magic_dns_name)"
    [[ "$https_port" != "443" ]] && suffix=":$https_port"
    if [[ -n "$name" ]]; then
        echo "HTTPS front: https://${name}${suffix} -> http://127.0.0.1:${port}"
    fi
    "$ts" serve status
}

serve_off() {
    "$ts" serve --https="$https_port" off
    echo "HTTPS front on :$https_port removed"
}

command=""
for arg in "$@"; do
    case "$arg" in
        install | install-update-agent | update | restart | status | logs | health | serve | serve-off)
            command="$arg"
            ;;
        -h | --help)
            usage
            exit 0
            ;;
    esac
done

if [[ -z "$command" ]]; then
    usage
    exit 1
fi

ts="$(resolve_tailscale)"

case "$command" in
    serve-off)
        serve_off
        ;;
    install)
        "$native" "$@"
        serve_on
        ;;
    status)
        "$native" "$@" || true
        echo "--- tailscale serve ---"
        "$ts" serve status || true
        ;;
    *)
        exec "$native" "$@"
        ;;
esac
