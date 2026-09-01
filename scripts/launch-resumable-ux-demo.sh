#!/usr/bin/env bash
set -euo pipefail

repo="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"

cargo build -q -p loopflow --bin lf --manifest-path "$repo/Cargo.toml"

binary="$repo/target/debug/lf"
read -r checksum size _ < <(cksum "$binary")
tmp_root="${TMPDIR:-/tmp}"
demo_home="${tmp_root%/}/lf-native-session-ux-${checksum}-${size}"
mkdir -p "$demo_home"

run_lf() {
  env \
    -u LF_BIN \
    -u LF_DB_PATH \
    -u LF_CONTROL_BIN \
    -u LF_CONTROL_HOME \
    -u LF_CONTROL_DB_PATH \
    -u LF_ACCOUNT_LEASE \
    -u LF_ACCOUNT_SELECTION \
    -u LF_RUN_ID \
    -u LF_RUN_DIR \
    -u LF_PARENT_RUN_ID \
    -u LF_PROVIDER_ACCOUNT_ID \
    RUST_LOG=warn \
    LF_HOME="$demo_home" \
    "$binary" "$@"
}

case "${1:-claude}" in
  claude|codex|opencode)
    agent="${1:-claude}"
    run_lf -i --tui -m "$agent" : "test"
    echo
    echo "Session saved. List it with:"
    echo "  scripts/launch-resumable-ux-demo.sh list"
    echo "Open it with:"
    echo "  scripts/launch-resumable-ux-demo.sh open <SESSION>"
    ;;
  list)
    run_lf session list
    ;;
  open)
    if [[ -z "${2:-}" ]]; then
      echo "usage: scripts/launch-resumable-ux-demo.sh open <SESSION> [--replace|--try]" >&2
      exit 2
    fi
    run_lf session open "${@:2}"
    ;;
  complete)
    if [[ -z "${2:-}" ]]; then
      echo "usage: scripts/launch-resumable-ux-demo.sh complete <SESSION>" >&2
      exit 2
    fi
    run_lf session complete "$2"
    ;;
  *)
    echo "usage: scripts/launch-resumable-ux-demo.sh [claude|codex|opencode|list|open <SESSION> [--replace|--try]|complete <SESSION>]" >&2
    exit 2
    ;;
esac
