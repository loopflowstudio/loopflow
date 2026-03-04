#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage: install-loopflow.sh [--check|--install] [--dry-run]

Install or verify loopflow base tooling inside agent images.

Modes:
  --check      report missing dependencies and exit non-zero (default)
  --install    install missing dependencies
  --dry-run    print what would be installed (implies no changes)
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

if [[ "$DRY_RUN" == "true" ]]; then
  MODE="install"
fi

missing=()
OPENCODE_VERSION="v1.2.17"
OPENCODE_SHA256_X64="dbfe556df45ac999eff95248269ccdd06ee2052983bb03b9501fe9dda2d1f695"
OPENCODE_SHA256_ARM64="a8c8958274c9b6d9939253b7779a8628c03ec34abbf874cfd5021dd1add12f83"

ensure_apt_package() {
  local package="$1"
  if dpkg -s "$package" >/dev/null 2>&1; then
    echo "✓ apt:$package"
    return
  fi
  missing+=("apt:$package")
}

ensure_command() {
  local name="$1"
  if command -v "$name" >/dev/null 2>&1; then
    echo "✓ $name"
    return
  fi
  missing+=("$name")
}

for package in git curl ca-certificates openssh-client jq; do
  ensure_apt_package "$package"
done

ensure_command claude
ensure_command codex
ensure_command gemini
ensure_command opencode

if [[ "${#missing[@]}" -eq 0 ]]; then
  exit 0
fi

if [[ "$MODE" == "check" ]]; then
  printf 'Missing loopflow base dependencies:\n' >&2
  printf '  - %s\n' "${missing[@]}" >&2
  exit 1
fi

if [[ "$DRY_RUN" == "true" ]]; then
  echo "would run apt-get install for missing apt dependencies"
  echo "would run npm install -g @anthropic-ai/claude-code @openai/codex @google/gemini-cli"
  echo "would download opencode ${OPENCODE_VERSION} binary from anomalyco/opencode releases and verify SHA256"
  exit 0
fi

export DEBIAN_FRONTEND=noninteractive
apt-get update
apt-get install -y --no-install-recommends \
  git \
  curl \
  ca-certificates \
  openssh-client \
  jq
rm -rf /var/lib/apt/lists/*

npm install -g \
  @anthropic-ai/claude-code \
  @openai/codex \
  @google/gemini-cli
npm cache clean --force

arch="$(dpkg --print-architecture)"
case "$arch" in
  amd64)
    oc_arch="x64"
    oc_sha256="${OPENCODE_SHA256_X64}"
    ;;
  arm64)
    oc_arch="arm64"
    oc_sha256="${OPENCODE_SHA256_ARM64}"
    ;;
  *)
    echo "unsupported arch: $arch" >&2
    exit 1
    ;;
esac

opencode_archive="$(mktemp)"
opencode_url="https://github.com/anomalyco/opencode/releases/download/${OPENCODE_VERSION}/opencode-linux-${oc_arch}.tar.gz"
if curl -fsSL "${opencode_url}" -o "${opencode_archive}"; then
  actual_sha256="$(sha256sum "${opencode_archive}" | awk '{print $1}')"
  if [[ "${actual_sha256}" != "${oc_sha256}" ]]; then
    echo "warning: opencode checksum mismatch (expected ${oc_sha256}, got ${actual_sha256})" >&2
  elif tar -xz -C /usr/local/bin -f "${opencode_archive}" opencode 2>/dev/null; then
    chmod +x /usr/local/bin/opencode
  else
    echo "warning: opencode archive extraction failed (optional)" >&2
  fi
else
  echo "warning: opencode download failed (optional)" >&2
fi
rm -f "${opencode_archive}"

git config --system init.defaultBranch main
git config --system safe.directory /workspace

# Install lf binary
case "$arch" in
  amd64) lf_arch="x86_64-unknown-linux-gnu" ;;
  arm64) lf_arch="aarch64-unknown-linux-gnu" ;;
  *)     lf_arch="" ;;
esac

if [[ -n "$lf_arch" ]]; then
  if curl -fsSL "https://github.com/loopflowstudio/loopflow/releases/latest/download/lf-${lf_arch}" \
    -o /usr/local/bin/lf 2>/dev/null; then
    chmod +x /usr/local/bin/lf
  else
    echo "warning: lf install failed (optional)" >&2
  fi
else
  echo "warning: lf not available for arch $arch (optional)" >&2
fi
