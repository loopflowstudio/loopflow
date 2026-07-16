#!/usr/bin/env bash
# Prove the stable Wave surface (W2-178) renders the five Proof states —
# selected, loading, error, empty, and future-child indentation — DISTINCTLY,
# at both a narrow and a wide desktop width, on a host without UI-automation
# permission.
#
# The permissioned XCUITest (LoopflowUITests/WaveSurfaceStateTests) asserts each
# state's unique affordance and row hittability; it runs on the maintained UI
# host. This script is the run-here complement: it launches the built app in
# each state, has the app render its own key window to a PNG (SnapshotService,
# no Screen Recording permission), and asserts the states produce pairwise
# distinct images. Together they catch a regression like PR #972's — where the
# detail-state env never reached the app, so loading and error rendered as
# selected (identical images here, failed affordance assertions there).
#
# Usage: scripts/prove_wave_surface_states.sh
set -euo pipefail

REPO="$(cd "$(dirname "$0")/.." && pwd)"
BIN="$REPO/swift/.build/debug/LoopflowMac"
if [ ! -x "$BIN" ]; then
  echo "Building LoopflowMac…"
  ( cd "$REPO/swift" && swift build --product LoopflowMac >/dev/null )
fi

OUT="$(mktemp -d -t wave_surface_states.XXXXXX)"
trap 'rm -rf "$OUT"' EXIT

# state|mode|detail_state|select_branch
STATES=(
  "selected|mock-waves||"
  "loading|mock-waves|loading|"
  "error|mock-waves|error|"
  "empty|empty-workspaces||"
  "child|mock-waves||cadenza"
)
WIDTHS=(900 1440)

capture() {
  local name="$1" mode="$2" detail="$3" branch="$4" width="$5" out="$6"
  LOOPFLOW_UI_TEST_DETAIL_STATE="$detail" \
  LOOPFLOW_UI_TEST_SELECT_BRANCH="$branch" \
  LOOPFLOW_UI_TEST_WIDTH="$width" \
  LOOPFLOW_UI_TEST_SNAPSHOT_PATH="$out" \
    "$BIN" -ui-test-mode "$mode" >/dev/null 2>&1 &
  local pid=$!
  # The app snapshots ~2.5s in, then self-terminates; wait it out with a cap.
  for _ in $(seq 1 20); do
    kill -0 "$pid" 2>/dev/null || break
    sleep 0.5
  done
  kill "$pid" 2>/dev/null || true
  wait "$pid" 2>/dev/null || true
}

echo "Capturing 5 states × 2 widths through the app's own renderer…"
declare -a HASHES=()
declare -a LABELS=()
fail=0
for width in "${WIDTHS[@]}"; do
  for entry in "${STATES[@]}"; do
    IFS='|' read -r name mode detail branch <<<"$entry"
    png="$OUT/${name}-${width}.png"
    capture "$name" "$mode" "$detail" "$branch" "$width" "$png"
    if [ ! -s "$png" ]; then
      echo "  FAIL — no snapshot for $name @ ${width}px"
      fail=1
      continue
    fi
    h="$(md5 -q "$png")"
    bytes="$(wc -c <"$png" | tr -d ' ')"
    echo "  $name @ ${width}px → ${bytes} bytes  ${h}"
    HASHES+=("$h")
    LABELS+=("${name}@${width}")
  done
done

# Every capture must be pairwise distinct: a collision means two states (or two
# widths) rendered identically — exactly the failure the forwarding bug caused.
echo "Checking all captures are pairwise distinct…"
n=${#HASHES[@]}
for ((i = 0; i < n; i++)); do
  for ((j = i + 1; j < n; j++)); do
    if [ "${HASHES[$i]}" = "${HASHES[$j]}" ]; then
      echo "  FAIL — ${LABELS[$i]} and ${LABELS[$j]} rendered identically (${HASHES[$i]})"
      fail=1
    fi
  done
done

if [ "$fail" -ne 0 ]; then
  echo "FAIL — the Wave surface did not render all states distinctly."
  exit 1
fi
echo "PASS — all ${n} captures (5 states × 2 widths) are distinct and non-empty."
