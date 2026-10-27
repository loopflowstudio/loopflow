#!/usr/bin/env bash
# Install the release jobs as ordinary crontab lines on the Wave's placed
# Machine. Each line runs a plain lf command; cron itself owns the schedule.
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
# Earlier lf scheduled these jobs as launchd agents; crontab replaces them.
for plist in "$HOME"/Library/LaunchAgents/loopflow.cron."$wave".*.plist; do
  [ -e "$plist" ] || continue
  label="$(basename "$plist" .plist)"
  launchctl bootout "gui/$(id -u)/$label" 2>/dev/null || true
  rm -f "$plist"
  printf 'removed launchd job %s\n' "$label"
done
log_dir="$repo_root/.lf/logs"
mkdir -p "$log_dir"
cron_env="HOME=$HOME PATH=$PATH LF_HOME=$lf_home TMPDIR=${TMPDIR:-/tmp} LANG=${LANG:-C}"
job() {
  printf '%s cd %q && env -i %s lf --wave %q --batch graph %s >> %q 2>&1 # loopflow:%s:%s\n' \
    "$1" "$repo_root" "$cron_env" "$wave" "$2" "$log_dir/$2.log" "$wave" "$2"
}
{
  crontab -l 2>/dev/null | grep -v "# loopflow:$wave:" || true
  job "0 9 * * *" telemetry-daily
  job "0 10 * * *" release-run
} | crontab -
crontab -l | grep "# loopflow:$wave:"

step "configured-path telemetry run"
if ! "${minimal_env[@]}" lf --wave "$wave" --batch graph telemetry-daily; then
  printf '\nbootstrap installed the jobs, but the telemetry run is red; inspect the output above\n' >&2
  exit 1
fi
printf '\nbootstrap complete: this Machine runs Wave %s release jobs from crontab\n' "$wave"
