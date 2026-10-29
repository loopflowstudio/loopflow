#!/usr/bin/env bash
# Install the release jobs on the Wave's placed Machine: gui-domain
# LaunchAgents on macOS, so they reach the login Keychain, and crontab lines
# elsewhere. Each schedule runs a generated wrapper around a plain lf command.
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
# The checks below and both job wrappers run in this one environment.
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
# Each schedule runs a generated wrapper that holds the full command: cron
# reads only 999 bytes of a command line, and the host PATH alone can exceed it.
cron_dir="$repo_root/.lf/cron"
mkdir -p "$log_dir" "$cron_dir"
cron_env="$(printf '%q ' "${minimal_env[@]}")"
wrapper() {
  local path="$cron_dir/$wave.$1.sh"
  printf '#!/bin/sh\ncd %q && exec %s%q --wave %q --batch graph %s >> %q 2>&1\n' \
    "$repo_root" "$cron_env" "$lf_bin" "$wave" "$1" "$log_dir/$1.log" >"$path"
  chmod 0755 "$path"
  printf '%s' "$path"
}

if [ "$(uname -s)" = Darwin ]; then
  # cron's jobs run outside the login session, where the login Keychain that
  # holds provider and publisher credentials is out of reach. A LaunchAgent in
  # the user's gui domain runs inside that session.
  uid="$(id -u)"
  agent_dir="$HOME/Library/LaunchAgents"
  mkdir -p "$agent_dir"
  xml() { sed -e 's/&/\&amp;/g' -e 's/</\&lt;/g' -e 's/>/\&gt;/g' <<<"$1"; }
  agent() {
    local label="loopflow.cron.$wave.$2" path plist
    path="$(wrapper "$2")"
    plist="$agent_dir/$label.plist"
    cat >"$plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key><string>$label</string>
  <key>ProgramArguments</key>
  <array><string>/bin/sh</string><string>$(xml "$path")</string></array>
  <key>StartCalendarInterval</key>
  <dict><key>Hour</key><integer>$1</integer><key>Minute</key><integer>0</integer></dict>
  <key>ProcessType</key><string>Standard</string>
  <key>AbandonProcessGroup</key><true/>
</dict>
</plist>
PLIST
    launchctl bootout "gui/$uid/$label" 2>/dev/null || true
    if ! launchctl bootstrap "gui/$uid" "$plist"; then
      printf 'could not load %s into gui/%s; log in to this Machine so its jobs reach the login Keychain\n' \
        "$label" "$uid" >&2
      return 1
    fi
    printf 'loaded %s: daily at %02d:00\n' "$label" "$1"
  }
  agent 9 telemetry-daily
  agent 10 release-run
  # Earlier bootstraps scheduled these jobs from crontab; one scheduler runs
  # each job.
  if crontab -l 2>/dev/null | grep -q "# loopflow:$wave:"; then
    { crontab -l | grep -v "# loopflow:$wave:" || true; } | crontab -
    printf 'removed crontab lines # loopflow:%s:\n' "$wave"
  fi
  run_telemetry() {
    local label="gui/$uid/loopflow.cron.$wave.telemetry-daily" info code
    # Started by launchd, the run has the same session a scheduled firing has.
    launchctl kickstart -k "$label" || return 1
    while :; do
      info="$(launchctl print "$label")" || return 1
      # The job's own fields carry one tab of indent; nested ones carry more.
      code="$(sed -n 's/^[[:space:]]last exit code = \([0-9][0-9]*\)$/\1/p' <<<"$info")"
      if ! grep -q '^[[:space:]]state = running' <<<"$info"; then
        [ -n "$code" ] && return "$code"
        # A job killed by a signal reports the signal instead of an exit code.
        grep -q '^[[:space:]]last terminating signal' <<<"$info" && return 1
      fi
      sleep 5
    done
  }
else
  job() {
    local path line bytes
    path="$(wrapper "$2")"
    line="$(printf '%s /bin/sh %q # loopflow:%s:%s' "$1" "$path" "$wave" "$2")"
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
      printf 'crontab is missing %s\n' "$name" >&2
      exit 1
    fi
  done
  run_telemetry() { /bin/sh "$cron_dir/$wave.telemetry-daily.sh"; }
fi

step "scheduled-path telemetry run"
if ! run_telemetry; then
  tail -n 40 "$log_dir/telemetry-daily.log" >&2 || true
  printf '\nbootstrap installed the jobs, but the telemetry run is red; inspect the log above\n' >&2
  exit 1
fi
tail -n 20 "$log_dir/telemetry-daily.log"
printf '\nbootstrap complete: this Machine runs Wave %s release jobs on schedule\n' "$wave"
