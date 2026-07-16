#!/usr/bin/env bash
# Capture the real Product Wave through the production registry path. This is
# intentionally separate from the deterministic CI fixture proof: an absent or
# incomplete PM snapshot is a named failure, never silently replaced by mocks.
#
# Usage: scripts/prove_product_wave_surface.sh [repo]
set -euo pipefail

REPO="$(cd "$(dirname "$0")/.." && pwd)"
TARGET_REPO="${1:-$REPO}"
BIN="$REPO/swift/.build/debug/LoopflowMac"
OUT="$REPO/.lf/tmp/wave-surface/product-live"
STATUS="$OUT/status.json"

mkdir -p "$OUT"
( cd "$TARGET_REPO" && lf status product --json ) >"$STATUS"

uv run python - "$STATUS" <<'PY'
import json
import sys
from pathlib import Path

payload = json.loads(Path(sys.argv[1]).read_text())
if payload["wave"]["name"] != "product":
    raise SystemExit("FAIL — lf status did not return the Product Wave")
if not payload["wave"]["goal"].strip():
    raise SystemExit("FAIL — Product has no objective")
projects = payload["projects"]
if not projects:
    raise SystemExit("FAIL — Product has no Projects")
if not any(project["project"]["krs"] for project in projects):
    raise SystemExit("FAIL — Product has no Project KR evidence")
if not any(
    not task["task"]["completed"]
    for project in projects
    for task in project["tasks"]
):
    raise SystemExit("FAIL — Product has no open Task rows to render")
print(
    f"Product registry proof: {len(projects)} Projects, "
    f"{sum(len(project['project']['krs']) for project in projects)} KRs"
)
PY

if [ ! -x "$BIN" ]; then
  ( cd "$REPO/swift" && swift build --product LoopflowMac >/dev/null )
fi

capture() {
  local width="$1" out="$OUT/product-${width}.png"
  rm -f "$out"
  LOOPFLOW_UI_TEST_SELECT_BRANCH=product \
  LOOPFLOW_UI_TEST_WIDTH="$width" \
  LOOPFLOW_UI_TEST_DELAY=6 \
  LOOPFLOW_UI_TEST_SNAPSHOT_PATH="$out" \
    "$BIN" --repo "$TARGET_REPO" -ui-test-mode live >/dev/null 2>&1 &
  local pid=$!
  for _ in $(seq 1 40); do
    kill -0 "$pid" 2>/dev/null || break
    sleep 0.5
  done
  kill "$pid" 2>/dev/null || true
  wait "$pid" 2>/dev/null || true
  if [ ! -s "$out" ]; then
    echo "FAIL — real Product snapshot was not created at ${width}px"
    exit 1
  fi
  echo "Product @ ${width}px → $out"
}

capture 900
capture 1440

if [ "$(md5 -q "$OUT/product-900.png")" = "$(md5 -q "$OUT/product-1440.png")" ]; then
  echo "FAIL — narrow and wide Product surfaces rendered identically"
  exit 1
fi

echo "PASS — real Product data rendered through the production registry path."
