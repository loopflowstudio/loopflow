#!/usr/bin/env bash
# Install the release cron jobs on the Wave's placed Machine. Crons are personal:
# this Machine installs them with `lf cron add`; they stay private until
# `lf cron publish <name>` shows one to teammates.
#
# Run this from the placed Machine after promoting a release `lf`. The script
# reconstructs the unattended environment from non-secret host-local paths;
# the Machine-local publisher command injects authority only into its child.
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

step "placed Machine"
local_machine="$(lf machine id)"
placed_machine="$(lf wave status "$wave" --json | jq -er '.wave.machine.id')"
if [ "$local_machine" != "$placed_machine" ]; then
  printf 'Wave %s is not placed on this Machine\n' "$wave" >&2
  exit 1
fi
printf 'Wave %s is placed on this Machine\n' "$wave"

lf_home="${LF_HOME:-$HOME/.lf}"
minimal_env=(
  env -i
  "HOME=$HOME"
  "USER=${USER:-}"
  "PATH=$PATH"
  "LF_HOME=$lf_home"
  "TMPDIR=${TMPDIR:-/tmp}"
  "LANG=${LANG:-C}"
)

step "unattended tool path"
"${minimal_env[@]}" sh -c '
  missing=0
  for tool in lf loopflow-release-publisher uv gh cargo flyctl security swift xcrun jq; do
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
auth_status="$("${minimal_env[@]}" lf account --json)"
printf '%s\n' "$auth_status"
if ! jq -e 'any(.accounts[]; .scope == "managed" and .verification == "accepted")' >/dev/null <<<"$auth_status"; then
  printf 'no managed provider account verified live from the Machine store\n' >&2
  exit 1
fi

step "configured publisher authority"
"${minimal_env[@]}" loopflow-release-publisher \
  uv run python scripts/publish_release.py check

step "install release schedules"
"${minimal_env[@]}" lf cron add telemetry-daily --schedule "0 0 9 * * *" \
  -- --wave "$wave" --batch graph telemetry-daily
"${minimal_env[@]}" lf cron add release-run --schedule "0 0 10 * * *" \
  -- --wave "$wave" --batch graph release-run

step "prove the release schedules are loaded here"
cron_json="$("${minimal_env[@]}" lf cron list --wave "$wave" --json)"
"${minimal_env[@]}" \
  "CRON_LIST_JSON=$cron_json" \
  "EXPECTED_MACHINE_ID=$local_machine" \
  uv run python - "$wave" <<'PY'
import json
import os
import sys

wave = sys.argv[1]
expected = [("release-run", "0 0 10 * * *"), ("telemetry-daily", "0 0 9 * * *")]
installed = json.loads(os.environ["CRON_LIST_JSON"])["installed"]
actual = [(entry["graph"], entry["schedule"]) for entry in installed]
if sorted(actual) != expected:
    raise SystemExit(f"cron drift: expected={expected!r} installed={sorted(actual)!r}")
for entry in installed:
    if entry["wave"] != wave:
        raise SystemExit(f"wrong Wave in installed cron: {entry['graph']}")
    if not entry["loaded"]:
        raise SystemExit(f"cron is not loaded: {entry['graph']}")
    if entry["machine_id"] != os.environ["EXPECTED_MACHINE_ID"]:
        raise SystemExit(f"wrong Machine in installed cron: {entry['graph']}")
print(f"{len(installed)}/{len(expected)} jobs loaded and exact")
PY

step "configured-path telemetry receipt"
result=0
if ! "${minimal_env[@]}" lf cron trigger \
  --wave "$wave" --graph telemetry-daily --wait --timeout 15m; then
  result=1
fi

step "configured-path release receipt"
if ! "${minimal_env[@]}" lf cron trigger \
  --wave "$wave" --graph release-run --wait --timeout 3h; then
  result=1
fi

step "35-day durable receipt window"
"${minimal_env[@]}" lf cron history --wave "$wave" --days 35

if [ "$result" -ne 0 ]; then
  printf '\nbootstrap installed the jobs, but a configured-path run is red; inspect the receipt and log above\n' >&2
  exit "$result"
fi
printf '\nbootstrap complete: this Machine owns Wave %s cron receipts\n' "$wave"
