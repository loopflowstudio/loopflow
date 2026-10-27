#!/usr/bin/env bash
# Install the release jobs as ordinary crontab lines on the Wave's placed
# Machine. Each line runs a generated wrapper around a plain lf command; cron
# itself owns the schedule.
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

step "installed lf"
# Promotion swaps the binary behind this entry gate; cron pins the gate.
lf_bin="$HOME/.lf-machine/install/gates/1/lf"
if [ ! -x "$lf_bin" ]; then
  printf 'no installed lf at %s; run lf self install first\n' "$lf_bin" >&2
  exit 1
fi
"$lf_bin" --version
host_path="$(dirname "$lf_bin"):$PATH"

step "placed Machine"
local_machine="$("$lf_bin" machine id)"
# A Wave with no Machine default runs on the Machine that reads it.
placed_machine="$("$lf_bin" wave status "$wave" --json | jq -er '.wave | select(.) | .machine.id // ""')"
if [ -z "$placed_machine" ]; then
  printf 'Wave %s has no Machine default; it runs on this Machine\n' "$wave"
elif [ "$placed_machine" != "$local_machine" ]; then
  printf 'Wave %s is placed on Machine %s\n' "$wave" "$placed_machine" >&2
  exit 1
else
  printf 'Wave %s is placed on this Machine\n' "$wave"
fi

lf_home="${LF_HOME:-$HOME/.lf}"
# The checks below and both cron lines run in this one environment.
minimal_env=(
  env -i
  "HOME=$HOME"
  "USER=${USER:-}"
  "PATH=$host_path"
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
auth_status="$("${minimal_env[@]}" "$lf_bin" account --json)"
printf '%s\n' "$auth_status"
if ! jq -e 'any(.accounts[]; .scope == "managed" and .verification == "accepted")' >/dev/null <<<"$auth_status"; then
  printf 'no managed provider account verified live from the Machine store\n' >&2
  exit 1
fi

step "configured publisher authority"
"${minimal_env[@]}" loopflow-release-publisher \
  uv run python scripts/publish_release.py check

step "install release schedules"
log_dir="$repo_root/.lf/logs"
# cron reads only 999 bytes of a command (Vixie MAX_COMMAND) and runs the cut
# prefix; the host PATH alone can exceed that. Each line runs a wrapper that
# holds the full command.
cron_dir="$repo_root/.lf/cron"
mkdir -p "$log_dir" "$cron_dir"
cron_env="$(printf '%q ' "${minimal_env[@]}")"
job() {
  local wrapper="$cron_dir/$wave.$2.sh" line bytes
  printf '#!/bin/sh\ncd %q && exec %s%q --wave %q --batch graph %s >> %q 2>&1\n' \
    "$repo_root" "$cron_env" "$lf_bin" "$wave" "$2" "$log_dir/$2.log" >"$wrapper"
  chmod 0755 "$wrapper"
  line="$(printf '%s /bin/sh %q # loopflow:%s:%s' "$1" "$wrapper" "$wave" "$2")"
  bytes="$(printf '%s' "$line" | wc -c)"
  if [ "$bytes" -ge 1000 ]; then
    printf 'crontab line for %s is %d bytes; cron runs only the first 999\n' "$2" "$bytes" >&2
    return 1
  fi
  printf '%s\n' "$line"
}
telemetry_line="$(job "0 9 * * *" telemetry-daily)"
release_line="$(job "0 10 * * *" release-run)"
{
  crontab -l 2>/dev/null | grep -v "# loopflow:$wave:" || true
  printf '%s\n' "$telemetry_line" "$release_line"
} | crontab -
installed="$(crontab -l | grep "# loopflow:$wave:")"
printf '%s\n' "$installed"
for name in telemetry-daily release-run; do
  if ! grep -q "# loopflow:$wave:$name\$" <<<"$installed"; then
    printf 'crontab is missing %s; launchd jobs left in place\n' "$name" >&2
    exit 1
  fi
done

# Earlier lf scheduled these jobs as launchd agents; remove them only now
# that the crontab replacement is confirmed.
for plist in "$HOME"/Library/LaunchAgents/loopflow.cron."$wave".*.plist; do
  [ -e "$plist" ] || continue
  label="$(basename "$plist" .plist)"
  launchctl bootout "gui/$(id -u)/$label" 2>/dev/null || true
  rm -f "$plist"
  printf 'removed launchd job %s\n' "$label"
done

step "configured-path telemetry run"
# Run the same wrapper cron runs.
if ! /bin/sh "$cron_dir/$wave.telemetry-daily.sh"; then
  tail -n 40 "$log_dir/telemetry-daily.log" >&2 || true
  printf '\nbootstrap installed the jobs, but the telemetry run is red; inspect the log above\n' >&2
  exit 1
fi
tail -n 20 "$log_dir/telemetry-daily.log"
printf '\nbootstrap complete: this Machine runs Wave %s release jobs from crontab\n' "$wave"
