#!/usr/bin/env bash
# Sandbox platform validation. Run on each target:
#   macOS (native), macOS (Concerto/DinD), Linux
#
# Usage: scripts/test_sandbox_platforms.sh

set -euo pipefail

CLEANUP_IDS=()
WORKSPACE=""
PROBE_WORKSPACE=""
SANDBOX_EXEC_LEGACY=false
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
  if [ -n "$PROBE_WORKSPACE" ] && [ -d "$PROBE_WORKSPACE" ]; then
    rm -rf "$PROBE_WORKSPACE"
  fi
}
trap cleanup EXIT

section() { printf '\n=== %s ===\n' "$1"; }
pass()    { printf '  PASS: %s\n' "$1"; }
fail()    { printf '  FAIL: %s\n' "$1"; exit 1; }

untrack_sandbox() {
  target="$1"
  for idx in "${!CLEANUP_IDS[@]}"; do
    if [ "${CLEANUP_IDS[$idx]}" = "$target" ]; then
      CLEANUP_IDS[$idx]=""
    fi
  done
}

run_sandbox_exec() {
  local sandbox_id="$1"
  shift
  if [ "$SANDBOX_EXEC_LEGACY" = "true" ]; then
    docker sandbox exec "$sandbox_id" -- "$@"
  else
    docker sandbox exec "$sandbox_id" "$@"
  fi
}

require_sandbox_cli_compatibility() {
  sandbox_help=$(docker sandbox --help 2>&1 || true)
  missing=()
  for command in create exec rm ls; do
    if ! printf '%s\n' "$sandbox_help" | grep -Eq "(^|[[:space:]])${command}([[:space:]]|$)"; then
      missing+=("$command")
    fi
  done

  if [ "${#missing[@]}" -gt 0 ]; then
    echo "  Detected sandbox help output:"
    printf '%s\n' "$sandbox_help" | sed 's/^/    /'
    fail "sandbox plugin missing required commands: ${missing[*]}"
  fi

  create_help=$(docker sandbox create --help 2>&1 || true)
  if ! printf '%s\n' "$create_help" | grep -Eq "(^|[[:space:]])claude([[:space:]]|$)"; then
    echo "  Detected sandbox create help output:"
    printf '%s\n' "$create_help" | sed 's/^/    /'
    fail "sandbox create command missing claude agent support"
  fi

  exec_help=$(docker sandbox exec --help 2>&1 || true)
  if printf '%s\n' "$exec_help" | grep -q "SANDBOX COMMAND \[ARG...\]"; then
    pass "sandbox exec supports direct command syntax"
    SANDBOX_EXEC_LEGACY=false
  elif printf '%s\n' "$exec_help" | grep -q "SANDBOX -- COMMAND"; then
    pass "sandbox exec uses legacy -- command separator syntax"
    SANDBOX_EXEC_LEGACY=true
  else
    echo "  Detected sandbox exec help output:"
    printf '%s\n' "$exec_help" | sed 's/^/    /'
    fail "sandbox exec usage not recognized"
  fi
}

create_local_workspace() {
  local prefix="$1"
  local template="${PWD}/.${prefix}.XXXXXX"
  mktemp -d "$template"
}

# ── Platform info ──

section "Platform"
echo "  OS:      $(uname -s) $(uname -m)"
echo "  Docker:  client=$(docker version --format '{{.Client.Version}}' 2>/dev/null || echo 'NOT AVAILABLE') server=$(docker version --format '{{.Server.Version}}' 2>/dev/null || echo 'NOT AVAILABLE')"

if ! sandbox_version=$(docker sandbox version 2>&1); then
  echo "  Sandbox: NOT AVAILABLE"
  fail "docker sandbox plugin not available"
fi
echo "  Sandbox:"
printf '%s\n' "$sandbox_version" | sed 's/^/    /'

require_sandbox_cli_compatibility

if ! docker info >/dev/null 2>&1; then
  fail "docker daemon not reachable (start Docker/OrbStack and retry)"
fi

# ── Startup probe ──

section "Startup Probe"
platform_id="lf-platform-test-$RUN_SUFFIX"
PROBE_WORKSPACE=$(create_local_workspace "lf-sandbox-probe")
docker sandbox create --name "$platform_id" claude "$PROBE_WORKSPACE"
CLEANUP_IDS+=("$platform_id")

result=$(run_sandbox_exec "$platform_id" echo "probe-ok" 2>&1)
if [ "$result" = "probe-ok" ]; then
  pass "create + exec lifecycle works"
else
  fail "exec returned unexpected output: $result"
fi

docker sandbox rm "$platform_id"
untrack_sandbox "$platform_id"

# ── Context file sync ──

section "Context File Sync"
WORKSPACE=$(create_local_workspace "lf-sandbox-context")
mkdir -p "$WORKSPACE/.lf/logs"
echo "test-context-content" > "$WORKSPACE/.lf/logs/test.context.md"
ctx_file_path="$WORKSPACE/.lf/logs/test.context.md"

ctx_id="lf-ctx-test-$RUN_SUFFIX"
docker sandbox create --name "$ctx_id" claude "$WORKSPACE"
CLEANUP_IDS+=("$ctx_id")

ctx_result=$(run_sandbox_exec "$ctx_id" cat "$ctx_file_path" 2>&1 || true)
if [ "$ctx_result" = "test-context-content" ]; then
  pass "context files visible inside sandbox"
else
  fail "context file not readable (got: $ctx_result)"
fi

docker sandbox rm "$ctx_id"
untrack_sandbox "$ctx_id"

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
