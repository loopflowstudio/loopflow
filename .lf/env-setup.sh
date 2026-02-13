#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage: .lf/env-setup.sh [--check|--install] [--dry-run]

Idempotent repo runtime setup for generated agent images.
This script is safe for all loopflow users and should avoid maintainer-only tooling.

Modes:
  --check      Report missing tools and exit non-zero if any are missing (default)
  --install    Install missing tools
  --dry-run    Print what would be installed (implies no changes)
USAGE
}

MODE="check"
DRY_RUN=false

while [[ $# -gt 0 ]]; do
  case "$1" in
    --check)
      MODE="check"
      shift
      ;;
    --install)
      MODE="install"
      shift
      ;;
    --dry-run)
      DRY_RUN=true
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "Unknown argument: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

if [[ "$MODE" == "install" && "$DRY_RUN" == "true" ]]; then
  echo "no-op: repo defines no extra runtime dependencies today"
  exit 0
fi

echo "no-op: repo defines no extra runtime dependencies today"

exit 0
