#!/usr/bin/env bash
# architecture-shim: local-refresh-wrapper
# Compatibility wrapper for the published release refresh path.
# Prefer: uv run python scripts/install.py refresh

set -euo pipefail

usage() {
    cat >&2 <<'USAGE'
Usage: pull-local-bin.sh [--repo PATH] [--install-dir PATH]

Compatibility wrapper around scripts/install.py refresh. Downloads and promotes
the latest published release through the external installer.
USAGE
}

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo="$(cd "$script_dir/.." && pwd)"
args=()

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
        --install-dir)
            if [[ $# -lt 2 ]]; then
                echo "--install-dir requires a path" >&2
                usage
                exit 1
            fi
            args+=("--install-dir" "$2")
            shift 2
            ;;
        --install-dir=*)
            args+=("--install-dir" "${1#--install-dir=}")
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
# bash 3.2 (macOS) treats an empty array as unbound under set -u
exec uv run --with typer python "$repo/scripts/install.py" refresh ${args[@]+"${args[@]}"}
