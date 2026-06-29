#!/usr/bin/env bash
# Manage the self-hosted loopflow server stack.
#
# Secrets come from Doppler when configured. Use LOOPFLOW_SECRETS=env to force
# plain Docker Compose with .env.

set -euo pipefail

usage() {
    cat >&2 <<'USAGE'
Usage: loopflow-server.sh [--repo PATH] <up|update|down|status|logs|health>

Commands:
  up       Build and start the self-hosted lfd stack
  update   Pull the default branch, then rebuild and restart the stack
  down     Stop the stack
  status   Show compose service status
  logs     Tail lfd and Caddy logs
  health   Check local lfd health

Environment:
  LOOPFLOW_SECRETS=auto|doppler|env   default: auto
  LF_DOMAIN=lfd.example.com           Caddy host name
  LF_TLS_MODE=internal                private/Tailscale TLS; leave empty for public ACME
  CADDYFILE=/path/to/Caddyfile        optional Caddy config override
  LFD_AUTH_TOKEN=...                  required when LFD_AUTH_MODE is local or unset
  LFD_PORT=2486                       local exposed lfd port
USAGE
}

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo="$(cd "$script_dir/.." && pwd)"
command=""

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
        -h|--help)
            usage
            exit 0
            ;;
        up|update|down|status|logs|health)
            command="$1"
            shift
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
project_name="$(basename "$repo" | tr '[:upper:]' '[:lower:]' | tr -c 'a-z0-9' '-')"
project_name="${project_name%-}"
compose=(docker compose -p "$project_name" -f "$repo/docker/docker-compose.yml" -f "$repo/deploy/docker-compose.prod.yml")

if [[ "${LF_TLS_MODE:-}" == "internal" && -z "${CADDYFILE:-}" ]]; then
    export CADDYFILE="$repo/deploy/Caddyfile.internal"
fi

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

preflight_remote_auth() {
    run_with_secrets bash -c '
        if [ "${LFD_AUTH_MODE:-local}" = "local" ] && [ -z "${LFD_AUTH_TOKEN:-}" ]; then
            echo "LFD_AUTH_TOKEN is required for self-hosted local auth" >&2
            echo "Set it in Doppler or export it before starting the server." >&2
            exit 1
        fi
    '
}

pull_default_branch() {
    local default_branch current_branch
    default_branch="$(git -C "$repo" symbolic-ref --quiet --short refs/remotes/origin/HEAD 2>/dev/null | sed 's#^origin/##')"
    current_branch="$(git -C "$repo" branch --show-current)"

    if [[ -n "$default_branch" && "$current_branch" != "$default_branch" ]]; then
        echo "refusing to update $current_branch; checkout $default_branch first" >&2
        exit 1
    fi

    if [[ -n "$default_branch" ]]; then
        git -C "$repo" pull --ff-only origin "$default_branch"
    else
        git -C "$repo" pull --ff-only
    fi
}

case "$command" in
    up)
        preflight_remote_auth
        run_with_secrets "${compose[@]}" up -d --build
        ;;
    update)
        pull_default_branch
        preflight_remote_auth
        run_with_secrets "${compose[@]}" up -d --build
        ;;
    down)
        run_with_secrets "${compose[@]}" down
        ;;
    status)
        run_with_secrets "${compose[@]}" ps
        ;;
    logs)
        run_with_secrets "${compose[@]}" logs --tail 200 -f lfd caddy
        ;;
    health)
        curl -fsS "http://127.0.0.1:${LFD_PORT:-2486}/health"
        echo
        ;;
esac
