#!/usr/bin/env bash
# Configure this Mac to use the Mac Mini self-hosted lfd over Tailscale.
# Stores the lfq environment in ~/.lf/mini.env and seeds Concerto's remote
# connection + Keychain token for the same host.

set -euo pipefail

usage() {
    cat >&2 <<'USAGE'
Usage: setup-mini-client.sh [options]

Options:
  --host HOST        Mac Mini Tailscale IP or MagicDNS name (default: 100.96.227.95)
  --ssh-user USER    SSH user for the mini alias (default: jack)
  --port PORT        lfd port (default: 2486)
  --https           Use https://HOST:PORT instead of http://HOST:PORT
  --token TOKEN      lfd bearer token. Defaults to LFD_TOKEN or LFD_AUTH_TOKEN.
  --token-file PATH  Read bearer token from PATH.
  --env-file PATH    Write shell exports here (default: ~/.lf/mini.env)
  --no-concerto      Do not seed Concerto UserDefaults/Keychain.
  --verify           Run lfq status after writing local config.
  -h, --help         Show this help.

The env file contains secrets and is written with 0600 permissions.
USAGE
}

host="100.96.227.95"
ssh_user="jack"
port="2486"
use_tls=0
token="${LFD_TOKEN:-${LFD_AUTH_TOKEN:-}}"
token_file=""
env_file="$HOME/.lf/mini.env"
configure_concerto=1
verify=0

while [[ $# -gt 0 ]]; do
    case "$1" in
        --host)
            host="$2"
            shift 2
            ;;
        --host=*)
            host="${1#--host=}"
            shift
            ;;
        --ssh-user)
            ssh_user="$2"
            shift 2
            ;;
        --ssh-user=*)
            ssh_user="${1#--ssh-user=}"
            shift
            ;;
        --port)
            port="$2"
            shift 2
            ;;
        --port=*)
            port="${1#--port=}"
            shift
            ;;
        --https)
            use_tls=1
            shift
            ;;
        --token)
            token="$2"
            shift 2
            ;;
        --token=*)
            token="${1#--token=}"
            shift
            ;;
        --token-file)
            token_file="$2"
            shift 2
            ;;
        --token-file=*)
            token_file="${1#--token-file=}"
            shift
            ;;
        --env-file)
            env_file="$2"
            shift 2
            ;;
        --env-file=*)
            env_file="${1#--env-file=}"
            shift
            ;;
        --no-concerto)
            configure_concerto=0
            shift
            ;;
        --verify)
            verify=1
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

if [[ -n "$token_file" ]]; then
    token="$(<"$token_file")"
fi

token="$(printf '%s' "$token" | tr -d '\r\n')"
if [[ -z "$token" ]]; then
    echo "LFD token is required. Pass --token, --token-file, or set LFD_TOKEN/LFD_AUTH_TOKEN." >&2
    exit 1
fi

if ! [[ "$port" =~ ^[0-9]+$ ]] || [[ "$port" -lt 1 || "$port" -gt 65535 ]]; then
    echo "invalid port: $port" >&2
    exit 1
fi

scheme="http"
if [[ "$use_tls" -eq 1 ]]; then
    scheme="https"
fi
url="$scheme://$host:$port"

mkdir -p "$(dirname "$env_file")"
tmp="$(mktemp)"
cat > "$tmp" <<EOF_ENV
# Source this file to point lfq and other loopflow clients at the Mac Mini lfd.
export MINI=$host
alias mini='ssh $ssh_user@\$MINI'
export LFD_URL=$url
export LFD_TOKEN=$token
EOF_ENV
install -m 0600 "$tmp" "$env_file"
rm -f "$tmp"

if [[ "$configure_concerto" -eq 1 ]]; then
    json="$(python3 - "$host" "$port" "$use_tls" <<'PY'
import json
import sys
host, port, use_tls = sys.argv[1], int(sys.argv[2]), sys.argv[3] == "1"
value = {
    "mode": "remote",
    "remoteConnection": {
        "host": host,
        "port": port,
        "useTLS": use_tls,
        "authMode": "staticToken",
    },
}
print(json.dumps(value, separators=(",", ":")))
PY
)"
    hex="$(printf '%s' "$json" | xxd -p -c 256)"
    defaults write com.loopflow.concerto concerto.connectionSettings.v2 -data "$hex"
    security add-generic-password \
        -U \
        -s loopflow.connection.token \
        -a "$host:$port" \
        -w "$token" >/dev/null
fi

echo "wrote $env_file"
echo "lfd url: $url"
echo "ssh alias: source $env_file && mini"
if [[ "$configure_concerto" -eq 1 ]]; then
    echo "seeded Concerto remote connection for $host:$port"
fi

if [[ "$verify" -eq 1 ]]; then
    # shellcheck disable=SC1090
    source "$env_file"
    lfq status
fi
