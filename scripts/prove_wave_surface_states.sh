#!/usr/bin/env bash
# Compare the loading and unavailable Wave surfaces at two desktop widths.
# The app renders its own windows, so no screen-recording permission is needed.
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

STATES=(loading error)
WIDTHS=(900 1440)

start_capture() {
  local state="$1" width="$2" out="$3"
  LOOPFLOW_UI_TEST_SELECT_BRANCH="" \
  LOOPFLOW_UI_TEST_DETAIL_STATE="$state" \
  LOOPFLOW_UI_TEST_WIDTH="$width" \
  LOOPFLOW_UI_TEST_SNAPSHOT_PATH="$out" \
    "$BIN" -ui-test-mode mock-waves >/dev/null 2>&1 &
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

echo "Capturing 2 states × 2 widths through the app's own renderer, two at a time…"
declare -a HASHES=()
declare -a LABELS=()
fail=0
for name in "${STATES[@]}"; do
  for width in "${WIDTHS[@]}"; do
    start_capture "$name" "$width" "$OUT/${name}-${width}.png"
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
echo "PASS — all ${n} captures (2 states × 2 widths) are distinct and non-empty."
