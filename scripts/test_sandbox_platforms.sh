#!/usr/bin/env bash
# Sandbox platform validation. Run on each target:
#   macOS (native), macOS (Concerto/DinD), Linux
#
# Usage: scripts/test_sandbox_platforms.sh

set -euo pipefail

CLEANUP_IDS=()
WORKSPACE=""
RUN_SUFFIX="${USER:-lf}-$$"

cleanup() {
  if [ "${#CLEANUP_IDS[@]}" -gt 0 ]; then
    for id in "${CLEANUP_IDS[@]}"; do
      [ -n "$id" ] || continue
      docker sandbox rm "$id" 2>/dev/null || true
    done
  fi
  if [ -n "$WORKSPACE" ] && [ -d "$WORKSPACE" ]; then
    rm -rf "$WORKSPACE"
  fi
}
trap cleanup EXIT

section() { printf '\n=== %s ===\n' "$1"; }
pass()    { printf '  PASS: %s\n' "$1"; }
fail()    { printf '  FAIL: %s\n' "$1"; exit 1; }
skip()    { printf '  SKIP: %s\n' "$1"; }

remove_cleanup_id() {
  target="$1"
  filtered=()
  for id in "${CLEANUP_IDS[@]}"; do
    if [ "$id" != "$target" ]; then
      filtered+=("$id")
    fi
  done
  CLEANUP_IDS=("${filtered[@]}")
}

require_sandbox_cli_compatibility() {
  help_output=$(docker sandbox --help 2>&1 || true)
  missing=()
  for command in create exec rm ls; do
    if ! printf '%s\n' "$help_output" | grep -Eq "(^|[[:space:]])${command}([[:space:]]|$)"; then
      missing+=("$command")
    fi
  done

  if [ "${#missing[@]}" -gt 0 ]; then
    echo "  Detected sandbox help output:"
    printf '%s\n' "$help_output" | sed 's/^/    /'
    fail "sandbox plugin missing required commands: ${missing[*]}"
  fi
}

# ── Platform info ──

section "Platform"
echo "  OS:      $(uname -s) $(uname -m)"
docker_client_version=$(docker version --format '{{.Client.Version}}' 2>/dev/null || true)
if [ -z "$docker_client_version" ]; then
  docker_client_version="NOT AVAILABLE"
fi
echo "  Docker:  $docker_client_version"

sandbox_version=$(docker sandbox version 2>&1 || true)
echo "  Sandbox: ${sandbox_version:-NOT AVAILABLE}"

if ! docker sandbox version >/dev/null 2>&1; then
  fail "docker sandbox plugin not available"
fi

require_sandbox_cli_compatibility

if ! docker info >/dev/null 2>&1; then
  fail "docker daemon not reachable (start Docker/OrbStack and retry)"
fi

# ── Startup probe ──

section "Startup Probe"
platform_id="lf-platform-test-$RUN_SUFFIX"
docker sandbox create --name "$platform_id" claude /tmp
CLEANUP_IDS+=("$platform_id")

result=$(docker sandbox exec "$platform_id" -- echo "probe-ok" 2>&1)
if [ "$result" = "probe-ok" ]; then
  pass "create + exec lifecycle works"
else
  fail "exec returned unexpected output: $result"
fi

docker sandbox rm "$platform_id"
remove_cleanup_id "$platform_id"

# ── Context file sync ──

section "Context File Sync"
WORKSPACE=$(mktemp -d)
mkdir -p "$WORKSPACE/.lf/logs"
echo "test-context-content" > "$WORKSPACE/.lf/logs/test.context.md"

ctx_id="lf-ctx-test-$RUN_SUFFIX"
docker sandbox create --name "$ctx_id" claude "$WORKSPACE"
CLEANUP_IDS+=("$ctx_id")

ctx_result=$(docker sandbox exec "$ctx_id" -- cat .lf/logs/test.context.md 2>&1 || true)
if [ "$ctx_result" = "test-context-content" ]; then
  pass "context files visible inside sandbox"
else
  fail "context file not readable (got: $ctx_result)"
fi

docker sandbox rm "$ctx_id"
remove_cleanup_id "$ctx_id"

# ── Gemini template probe ──

section "Gemini Template Probe"
gemini_id="lf-gemini-test-$RUN_SUFFIX"
docker sandbox create --name "$gemini_id" claude /tmp
CLEANUP_IDS+=("$gemini_id")

gemini_path=$(docker sandbox exec "$gemini_id" -- which gemini 2>/dev/null || true)
if [ -n "$gemini_path" ]; then
  gemini_ver=$(docker sandbox exec "$gemini_id" -- gemini --version 2>/dev/null || echo "unknown")
  pass "gemini CLI found at $gemini_path (version: $gemini_ver)"
else
  echo "  NOT FOUND: gemini CLI not in claude sandbox template"
  echo "  Next step: install via 'npm install -g @google/gemini-cli' or custom template"

  # Try npm install path
  install_result=$(docker sandbox exec "$gemini_id" -- npm install -g @google/gemini-cli 2>&1 || true)
  gemini_check=$(docker sandbox exec "$gemini_id" -- which gemini 2>/dev/null || true)
  if [ -n "$gemini_check" ]; then
    pass "gemini CLI installable via npm inside sandbox"
  else
    skip "gemini CLI not installable (npm output: ${install_result:0:200})"
  fi
fi

docker sandbox rm "$gemini_id"
remove_cleanup_id "$gemini_id"

# ── Cleanup verification ──

section "Cleanup Verification"
remaining=$(docker sandbox ls --quiet 2>/dev/null | grep "^lf-" || true)
if [ -n "$remaining" ]; then
  fail "orphaned sandboxes remain: $remaining"
fi
pass "no orphaned lf-* sandboxes"

# ── Done ──

section "RESULT"
echo "  ALL CHECKS PASSED"
