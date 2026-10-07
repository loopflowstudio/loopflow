#!/usr/bin/env bash
# Prove Loopflow Desktop renders its four fixture states — populated,
# loading, unavailable, and empty — DISTINCTLY at both a narrow and a wide
# desktop width, on a host without UI-automation permission.
#
# The permissioned XCUITest asserts each state's unique affordance and region
# reachability; it runs on the maintained UI host. This script is the run-here
# complement: it launches the built app in each state, has the app render its
# own key window to a PNG (SnapshotService, no Screen Recording permission),
# and asserts the states produce pairwise distinct images.
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
PIDS=()
stop_captures() {
  local pid
  if [ "${#PIDS[@]}" -gt 0 ]; then
    for pid in "${PIDS[@]}"; do kill "$pid" 2>/dev/null || true; done
    for pid in "${PIDS[@]}"; do wait "$pid" 2>/dev/null || true; done
  fi
  PIDS=()
}
cleanup() {
  stop_captures
  rm -rf "$OUT"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

# state|mode|detail_state|select_branch
STATES=(
  "populated|mock-waves||"
  "loading|mock-waves|loading|"
  "unavailable|mock-waves|error|"
  "empty|empty-workspaces||"
)
WIDTHS=(900 1440)

start_capture() {
  local mode="$1" detail="$2" branch="$3" width="$4" out="$5"
  LOOPFLOW_UI_TEST_DETAIL_STATE="$detail" \
  LOOPFLOW_UI_TEST_SELECT_BRANCH="$branch" \
  LOOPFLOW_UI_TEST_WIDTH="$width" \
  LOOPFLOW_UI_TEST_SNAPSHOT_PATH="$out" \
    "$BIN" -ui-test-mode "$mode" >/dev/null 2>&1 &
  PIDS+=("$!")
}

wait_for_captures() {
  local pid alive
  # The app snapshots ~2.5s in, then self-terminates; wait it out with a cap
  # generous enough for a cold CI runner's first render.
  for _ in $(seq 1 60); do
    alive=0
    for pid in "${PIDS[@]}"; do
      if kill -0 "$pid" 2>/dev/null; then alive=1; fi
    done
    if [ "$alive" -eq 0 ]; then break; fi
    sleep 0.5
  done
  stop_captures
}

echo "Capturing 4 states × 2 widths through the app's own renderer, two at a time…"
declare -a HASHES=()
declare -a LABELS=()
fail=0
for entry in "${STATES[@]}"; do
  IFS='|' read -r name mode detail branch <<<"$entry"
  for width in "${WIDTHS[@]}"; do
    start_capture "$mode" "$detail" "$branch" "$width" "$OUT/${name}-${width}.png"
  done
  wait_for_captures
  for width in "${WIDTHS[@]}"; do
    png="$OUT/${name}-${width}.png"
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
  echo "FAIL — Loopflow Desktop did not render all states distinctly."
  exit 1
fi
echo "PASS — all ${n} captures (4 states × 2 widths) are distinct and non-empty."
