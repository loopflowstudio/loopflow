#!/usr/bin/env bash
# Prove the AttributeGraph zero-cycle acceptance criterion for the Mac surface
# (W2-178). Launches the built app, captures its unified-log stream, and fails if
# any "AttributeGraph: cycle detected" line appears while you exercise the
# interaction matrix.
#
# SwiftUI reports dependency cycles to the unified logging system (os_log) and
# can also print them to the app's stderr, so the capture reads both: `log
# stream` on the app process and the app's own output share one log.
#
# Usage:
#   scripts/check_attributegraph_cycles.sh [--repo <path> | --sheet] [--seconds N]
#
# With --sheet the app opens Portfolio, selects the infrastructure fixture Wave
# and presents the create sheet. Otherwise exercise the window during capture.
set -euo pipefail

REPO="$(cd "$(dirname "$0")/.." && pwd)"
SECONDS_TO_CAPTURE=15
TARGET_REPO="$REPO"
SHEET=0

while [ $# -gt 0 ]; do
  case "$1" in
    --repo) TARGET_REPO="$2"; shift 2 ;;
    --seconds) SECONDS_TO_CAPTURE="$2"; shift 2 ;;
    --sheet) SHEET=1; shift ;;
    *) echo "unknown arg: $1"; exit 2 ;;
  esac
done

# The Xcode build bundles this checkout's lf at Contents/MacOS/lf, which the
# app requires (it never falls back to PATH). Same derived data as the
# screenshot pipeline; it leaves an installed Loopflow Dev.app alone.
APP="$REPO/swift/.build/xcode-derived-data/Build/Products/Debug/Loopflow.app"
BIN="$APP/Contents/MacOS/Loopflow"
echo "Building Loopflow.app…"
( cd "$REPO/swift" && xcodegen generate >/dev/null && xcodebuild build \
    -project LoopflowSwift.xcodeproj -scheme LoopflowMac -destination 'platform=macOS' \
    -derivedDataPath .build/xcode-derived-data -disableAutomaticPackageResolution \
    CODE_SIGNING_ALLOWED=YES CODE_SIGNING_REQUIRED=YES CODE_SIGN_STYLE=Manual \
    CODE_SIGN_IDENTITY=- DEVELOPMENT_TEAM= >/dev/null )

LOG="$(mktemp -t lf_attributegraph.XXXXXX.log)"
/usr/bin/log stream --style compact --process Loopflow >>"$LOG" 2>&1 &
LOG_PID=$!
sleep 1.5

if [ "$SHEET" -eq 1 ]; then
  LOOPFLOW_UI_TEST_VIEW=portfolio LOOPFLOW_UI_TEST_PRESENT_SHEET=1 \
    LOOPFLOW_UI_TEST_SELECT_BRANCH=infrastructure \
    "$BIN" -ui-test-mode mock-waves >>"$LOG" 2>&1 &
else
  "$BIN" --repo "$TARGET_REPO" >>"$LOG" 2>&1 &
fi
APP_PID=$!

if [ "$SHEET" -eq 1 ]; then
  echo "Checking sheet presentation over the infrastructure Wave."
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
if [ "$SHEET" -eq 1 ] && ! grep -Fxq "Create sheet appeared over infrastructure" "$LOG"; then
  echo "FAIL — create sheet did not appear over the selected Wave. See $LOG"
  exit 1
fi
echo "PASS — zero AttributeGraph cycles across the captured window."
rm -f "$LOG"
