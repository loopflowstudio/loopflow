#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage: .lf/env-setup.sh [--check|--install] [--dry-run]

Idempotent loopflow maintainer environment setup.

Modes:
  --check      Report missing tools and exit non-zero if any are missing (default)
  --install    Install missing tools (Homebrew on macOS)
  --dry-run    Print what would be installed (implies no changes)
USAGE
}

has_command() {
  command -v "$1" >/dev/null 2>&1
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

require_homebrew() {
  if has_command brew; then
    return
  fi

  if [[ "$MODE" == "install" ]]; then
    echo "Homebrew is required for --install on macOS. Install from https://brew.sh" >&2
  else
    echo "Homebrew not found. Install from https://brew.sh" >&2
  fi
  exit 1
}

ensure_brew_formula() {
  local cmd="$1"
  local formula="$2"
  local missing_ref_name="$3"

  if has_command "$cmd"; then
    echo "✓ $cmd"
    return
  fi

  if [[ "$MODE" == "install" ]]; then
    if [[ "$DRY_RUN" == "true" ]]; then
      echo "would install: brew install $formula"
      return
    fi

    brew install "$formula"
    if has_command "$cmd"; then
      echo "✓ $cmd"
      return
    fi
  fi

  printf -v "$missing_ref_name" '%s\n%s' "${!missing_ref_name}" "$cmd"
  echo "- missing: $cmd"
}

if [[ "$(uname -s)" != "Darwin" ]]; then
  if [[ "$MODE" == "install" ]]; then
    echo "Maintainer tool auto-install currently supports macOS only." >&2
    exit 1
  fi
fi

require_homebrew

MISSING_TOOLS=""
ensure_brew_formula "gh" "gh" MISSING_TOOLS
ensure_brew_formula "doppler" "dopplerhq/cli/doppler" MISSING_TOOLS
ensure_brew_formula "cloudflared" "cloudflared" MISSING_TOOLS

if [[ "$MODE" == "check" && -n "$MISSING_TOOLS" ]]; then
  echo
  echo "Missing tools:${MISSING_TOOLS}"
  echo "Run: .lf/env-setup.sh --install"
  exit 1
fi

if [[ "$MODE" == "install" && "$DRY_RUN" == "true" ]]; then
  exit 0
fi

if [[ "$MODE" == "install" && -n "$MISSING_TOOLS" ]]; then
  echo
  echo "Still missing after install attempt:${MISSING_TOOLS}" >&2
  exit 1
fi

exit 0
