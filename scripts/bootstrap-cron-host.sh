#!/usr/bin/env bash
# Bootstrap repo-owned cron jobs on the Wave's placed Home.
#
# Run this from the placed Home after promoting a release `lf`. The script
# reconstructs the unattended environment from non-secret host-local paths;
# Doppler injects publisher secrets only into its read-only preflight.
#
# Usage: scripts/bootstrap-cron-host.sh [wave]
#   scripts/bootstrap-cron-host.sh infrastructure
set -euo pipefail

wave="${1:-infrastructure}"
repo_root="$(git rev-parse --show-toplevel)"
main_repo="$(dirname "$(git rev-parse --path-format=absolute --git-common-dir)")"
if [ "$repo_root" != "$main_repo" ]; then
  printf 'run bootstrap from the authoritative main checkout %s, not %s\n' \
    "$main_repo" "$repo_root" >&2
  exit 1
fi
cd "$repo_root"

step() { printf '\n== %s ==\n' "$1"; }

step "placed Home"
local_home="$(lf home id)"
placed_home="$(lf status "$wave" --json | jq -er '.wave.home.id')"
if [ "$local_home" != "$placed_home" ]; then
  printf 'Wave %s is placed on %s, not local Home %s\n' \
    "$wave" "$placed_home" "$local_home" >&2
  exit 1
fi
printf '%s\n' "$local_home"

lf_home="${LF_CONTROL_HOME:-${LF_HOME:-$HOME/.lf}}"
lf_db_path="${LF_CONTROL_DB_PATH:-${LF_DB_PATH:-$lf_home/loopflow.db}}"
minimal_env=(
  env -i
  "HOME=$HOME"
  "USER=${USER:-}"
  "PATH=$PATH"
  "LF_HOME=$lf_home"
  "LF_DB_PATH=$lf_db_path"
  "TMPDIR=${TMPDIR:-/tmp}"
  "LANG=${LANG:-C}"
)

step "installed binary + declared jobs"
"${minimal_env[@]}" lf cron preflight --wave "$wave"

step "unattended tool path"
"${minimal_env[@]}" sh -c '
  missing=0
  for tool in lf doppler uv gh cargo flyctl security swift xcrun jq; do
    if ! command -v "$tool" >/dev/null 2>&1; then
      printf "missing: %s\n" "$tool" >&2
      missing=1
    fi
  done
  if ! command -v codex >/dev/null 2>&1 && ! command -v claude >/dev/null 2>&1; then
    printf "missing: codex or claude provider CLI\n" >&2
    missing=1
  fi
  exit "$missing"
'
step "host-local provider authority"
auth_status="$("${minimal_env[@]}" lf auth accounts --verify)"
printf '%s\n' "$auth_status"
if ! grep -q 'live active' <<<"$auth_status"; then
  printf 'no managed provider account verified live from the Home store\n' >&2
  exit 1
fi

step "Doppler publisher authority"
"${minimal_env[@]}" doppler run \
  --project loopflow \
  --config prd \
  -- \
  uv run python scripts/publish_release.py check

step "sync repo-owned schedules"
"${minimal_env[@]}" lf cron sync --wave "$wave"

step "prove GOAL.md matches loaded launchd jobs"
cron_json="$("${minimal_env[@]}" lf cron list --wave "$wave" --json)"
"${minimal_env[@]}" \
  "CRON_LIST_JSON=$cron_json" \
  "EXPECTED_HOME_ID=$local_home" \
  uv run python - "$wave" <<'PY'
import json
import os
import sys
from pathlib import Path

import yaml

wave = sys.argv[1]
goal = Path("wave") / wave / "GOAL.md"
frontmatter = yaml.safe_load(goal.read_text().split("---", 2)[1])
expected = [
    (entry["flow"], entry["schedule"])
    for entry in frontmatter.get("crons", [])
]
installed = json.loads(os.environ["CRON_LIST_JSON"])
actual = [(entry["flow"], entry["schedule"]) for entry in installed]
if sorted(actual) != sorted(expected):
    raise SystemExit(f"cron drift: GOAL.md={sorted(expected)!r} installed={sorted(actual)!r}")
for entry in installed:
    if entry["wave"] != wave:
        raise SystemExit(f"wrong Wave in installed cron: {entry!r}")
    if not entry["loaded"]:
        raise SystemExit(f"cron is not loaded: {entry['flow']}")
    if entry["home_id"] != os.environ["EXPECTED_HOME_ID"]:
        raise SystemExit(f"wrong Home in installed cron: {entry!r}")
print(f"{len(installed)}/{len(expected)} jobs loaded and exact")
PY

step "configured-path telemetry receipt"
result=0
if ! "${minimal_env[@]}" lf cron trigger \
  --wave "$wave" --flow telemetry-daily --wait --timeout 15m; then
  result=1
fi

step "configured-path release receipt"
if ! "${minimal_env[@]}" lf cron trigger \
  --wave "$wave" --flow release-run --wait --timeout 3h; then
  result=1
fi

step "35-day durable receipt window"
"${minimal_env[@]}" lf cron history --wave "$wave" --days 35

if [ "$result" -ne 0 ]; then
  printf '\nbootstrap installed the jobs, but a configured-path run is red; inspect the receipt and log above\n' >&2
  exit "$result"
fi
printf '\nbootstrap complete: %s owns Wave %s cron receipts\n' "$local_home" "$wave"
