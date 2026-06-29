#!/usr/bin/env bash
# Pull the default branch, build lf/lfd, and install them into a local bin directory.
#
# Usage:
#   scripts/pull-local-bin.sh
#   scripts/pull-local-bin.sh --no-pull
#   scripts/pull-local-bin.sh --repo ~/src/loopflow --install-dir ~/.local/bin

set -euo pipefail

usage() {
    cat >&2 <<'USAGE'
Usage: pull-local-bin.sh [--repo PATH] [--install-dir PATH] [--no-pull]

Builds release lf/lfd from the repo and atomically copies them into the install
bin directory. Defaults: current script's repo and $LF_INSTALL_DIR or ~/.local/bin.
USAGE
}

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo="$(cd "$script_dir/.." && pwd)"
install_dir="${LF_INSTALL_DIR:-$HOME/.local/bin}"
pull=true

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
            install_dir="$2"
            shift 2
            ;;
        --install-dir=*)
            install_dir="${1#--install-dir=}"
            shift
            ;;
        --no-pull)
            pull=false
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
install_dir="$(mkdir -p "$install_dir" && cd "$install_dir" && pwd)"

if [[ "$pull" == true ]]; then
    default_branch="$(git -C "$repo" symbolic-ref --quiet --short refs/remotes/origin/HEAD 2>/dev/null | sed 's#^origin/##')"
    current_branch="$(git -C "$repo" branch --show-current)"
    if [[ -n "$default_branch" && "$current_branch" != "$default_branch" ]]; then
        echo "refusing to pull $current_branch; checkout $default_branch or pass --no-pull" >&2
        exit 1
    fi

    echo "pulling $repo"
    if [[ -n "$default_branch" ]]; then
        git -C "$repo" pull --ff-only origin "$default_branch"
    else
        git -C "$repo" pull --ff-only
    fi
fi

echo "building lf/lfd"
cargo build --release -p loopflow --bin lf --bin lfd --manifest-path "$repo/Cargo.toml"

for bin in lf lfd; do
    src="$repo/target/release/$bin"
    dst="$install_dir/$bin"
    tmp="$install_dir/.${bin}.tmp.$$"
    if [[ ! -x "$src" ]]; then
        echo "missing built binary: $src" >&2
        exit 1
    fi
    cp "$src" "$tmp"
    chmod +x "$tmp"
    mv -f "$tmp" "$dst"
done

echo "installed: $install_dir/lf"
"$install_dir/lf" --version
"$install_dir/lfd" --version
