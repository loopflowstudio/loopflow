#!/usr/bin/env bash
# Prove the AttributeGraph zero-cycle acceptance criterion for the Mac surface
# (W2-178). Launches the built app, captures its unified-log stream, and fails if
# any "AttributeGraph: cycle detected" line appears while you exercise the
# interaction matrix.
#
# SwiftUI logs dependency cycles to the unified logging system (os_log), not
# stderr — so `log stream` on the app process is the authoritative source. An
# empty stderr proves nothing; this script reads the right place.
#
# Usage:
#   scripts/check_attributegraph_cycles.sh [--repo <path>] [--seconds N]
#
# Cold launch, repository switch, refresh, and Chat-in-the-default-pane are all
# exercised by the RoadmapView default surface that renders at launch. To cover
# Wave selection, WaveChat selection, and sheet/dialog presentation, interact
# with the window while the capture runs (the script prints a countdown).
set -euo pipefail

REPO="$(cd "$(dirname "$0")/.." && pwd)"
SECONDS_TO_CAPTURE=15
TARGET_REPO="$REPO"
MOCK=0

while [ $# -gt 0 ]; do
  case "$1" in
    --repo) TARGET_REPO="$2"; shift 2 ;;
    --seconds) SECONDS_TO_CAPTURE="$2"; shift 2 ;;
    # --mock launches the `mock-waves` UI-test mode: a fixture-seeded populated
    # Wave surface, auto-selected into its full detail hierarchy, with NO
    # registry. This drives the WaveDetailPane selection + Chat-render path (the
    # @StateObject->@ObservedObject fix site) at cold launch on a machine whose
    # `lf` can't serve W2-123 lens data — no interaction needed.
    --mock) MOCK=1; shift ;;
    *) echo "unknown arg: $1"; exit 2 ;;
  esac
done

BIN="$REPO/swift/.build/debug/LoopflowMac"
if [ ! -x "$BIN" ]; then
  echo "Building LoopflowMac…"
  ( cd "$REPO/swift" && swift build --product LoopflowMac >/dev/null )
fi

LOG="$(mktemp -t lf_attributegraph.XXXXXX.log)"
/usr/bin/log stream --style compact --process LoopflowMac >"$LOG" 2>&1 &
LOG_PID=$!
sleep 1.5

if [ "$MOCK" -eq 1 ]; then
  "$BIN" -ui-test-mode mock-waves >/dev/null 2>&1 &
else
  "$BIN" --repo "$TARGET_REPO" >/dev/null 2>&1 &
fi
APP_PID=$!

if [ "$MOCK" -eq 1 ]; then
  echo "App launched (pid $APP_PID) in mock-waves mode — a populated Wave is"
  echo "auto-selected, so cold launch + Wave selection + Chat render are captured"
  echo "with no interaction. Open a sheet during the window to also cover that."
else
  echo "App launched (pid $APP_PID). Exercise the matrix now:"
  echo "  cold launch · repo switch · Wave select · refresh · Chat select · open a sheet/dialog"
fi
for ((s=SECONDS_TO_CAPTURE; s>0; s--)); do printf "\r  capturing… %2ds " "$s"; sleep 1; done
printf "\n"

kill "$APP_PID" 2>/dev/null || true; wait "$APP_PID" 2>/dev/null || true
sleep 1
kill "$LOG_PID" 2>/dev/null || true; wait "$LOG_PID" 2>/dev/null || true

COUNT="$(grep -iEc 'cycle detected|AttributeGraph' "$LOG" || true)"
echo "Captured $(wc -l <"$LOG") log lines; AttributeGraph/cycle mentions: $COUNT"
if [ "$COUNT" -gt 0 ]; then
  echo "FAIL — AttributeGraph cycles detected:"
  grep -iE 'cycle detected|AttributeGraph' "$LOG" | head
  echo "log: $LOG"
  exit 1
fi
echo "PASS — zero AttributeGraph cycles across the captured window."
rm -f "$LOG"
