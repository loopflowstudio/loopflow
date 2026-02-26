#!/usr/bin/env bash
set -euo pipefail

# Validate Stage 02: Mobile Action Buttons
# Runs automated checks, then launches Concerto for manual walkthrough.

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_ROOT"

PASS=0
FAIL=0

run_check() {
    local label="$1"
    shift
    printf "  %-40s " "$label"
    if "$@" >/dev/null 2>&1; then
        echo "✓"
        PASS=$((PASS + 1))
    else
        echo "✗"
        FAIL=$((FAIL + 1))
    fi
}

echo "=== Automated checks ==="
echo ""

run_check "cargo fmt" cargo fmt --all -- --check
run_check "cargo clippy" cargo clippy --all-targets -- -D warnings
run_check "cargo test (skip docker)" cargo test --all \
    -- --skip lfd::executor::docker::tests::docker_startup_lost_agent_does_not_flip_terminal_run_wave_status \
       --skip lfd::executor::docker::tests::docker_startup_rehydrates_running_agents_and_cleans_orphans
run_check "python tests" uv run pytest python/tests/ -q
run_check "swift tests" swift test --package-path swift

echo ""
echo "=== Results: $PASS passed, $FAIL failed ==="
echo ""

if [ "$FAIL" -gt 0 ]; then
    echo "Fix failures before manual verification."
    exit 1
fi

echo "=== Manual walkthrough ==="
echo ""
echo "The following will launch Concerto in debug mode."
echo "Once running, verify:"
echo ""
echo "  1. Open a wave chat session (any wave)"
echo "  2. Send a message and wait for the agent to respond"
echo "  3. After the agent completes a turn, action buttons should appear"
echo "     above the composer"
echo "  4. On compact/iPhone layout: buttons stack vertically, max 3"
echo "  5. On regular layout: buttons use adaptive grid, max 4"
echo "  6. Tap a button → it sends as a user message, buttons clear"
echo "  7. Start typing in the composer → buttons clear"
echo "  8. New agent turn starts → buttons clear"
echo "  9. End session → buttons clear"
echo ""
echo "Press Enter to launch Concerto, or Ctrl-C to skip."
read -r

exec uv run python scripts/concerto-dev.py run-debug
