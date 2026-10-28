#!/usr/bin/env bash
# Disk cleanup for Jack's machines (the laptop and mini-heart): the mechanical
# half of the disk-cleanup skill (.lf/skills/disk-cleanup.md). Cron runs the
# skill, the skill runs this, then reports what this freed and kept.
#
# Every repository under ~/src: finished worktrees (`lf wt prune`), completed
# Tasks' build output (`lf wt prune --builds`), merged branches
# (`lf wt prune --branches`) and build output in every checkout once nothing
# has written to it for 4 hours.
# Home (~/.lf): captures, logs, retired draft store copies and all but the newest backup
# per database, once untouched for a week. Machine caches: uv, unavailable
# simulators, idle ~/Library/Caches entries and old Trash.
#
# Never removes the database or its sidecars, account homes, the newest backup
# per database, Home material modified within the last day, or anything a process
# holds open; when lsof fails, nothing that check protects is removed. Each
# removal is appended to ~/.lf/logs/disk.log. This script does all the deleting;
# the skill only reads its output and reports.
#
# Usage:
#   scripts/disk-cleanup.sh              run once
#   scripts/disk-cleanup.sh install      run the skill hourly from crontab
#   scripts/disk-cleanup.sh uninstall
#   scripts/disk-cleanup.sh skill        what cron runs: the skill, unattended
#
# Logs: ~/Library/Logs/loopflow-disk-cleanup.log (this script),
# loopflow-disk-cleanup-skill.log (the skill's answer under cron)
set -uo pipefail

marker="# loopflow:disk-cleanup"
log="$HOME/Library/Logs/loopflow-disk-cleanup.log"
src="${LOOPFLOW_SRC:-$HOME/src}"
lf_home="${LF_HOME:-$HOME/.lf}"
disk_log="$lf_home/logs/disk.log"
home_keep_days=7
build_idle_minutes=240
uv_clean_over_gib=10
cache_idle_days=7
trash_idle_days=30

# --------------------------------------------------------------------------
# install / uninstall / skill

without_entry() { crontab -l 2>/dev/null | grep -vF "$marker" || true; }

main_checkout() {
  local repo
  repo="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
  dirname "$(git -C "$repo" rev-parse --path-format=absolute --git-common-dir)"
}

install_entry() {
  local main
  main="$(main_checkout)"
  if [ ! -f "$main/.lf/skills/disk-cleanup.md" ]; then
    printf 'install from a main checkout that has .lf/skills/disk-cleanup.md (land LOO-647 first)\n' >&2
    exit 1
  fi
  # cron runs only the first 999 bytes of a command; the environment lives in
  # `skill` below, so the line stays short.
  {
    without_entry
    printf '17 * * * * /bin/bash %q skill %s\n' "$main/scripts/disk-cleanup.sh" "$marker"
  } | crontab - || return 1
  crontab -l | grep -F "$marker"
  # The launchd agent this replaces.
  launchctl bootout "gui/$(id -u)/com.loopflow.disk-cleanup" 2>/dev/null || true
  rm -f "$HOME/Library/LaunchAgents/com.loopflow.disk-cleanup.plist"
}

# Run the skill from the main checkout in the minimal environment the release
# jobs use (bootstrap-cron-host.sh), with the installed gate's lf.
run_skill() {
  local lf_bin
  lf_bin="$HOME/.lf-machine/install/gates/1/lf"
  [ -x "$lf_bin" ] || lf_bin="$(command -v lf)" || { echo "no lf installed" >&2; exit 1; }
  mkdir -p "$(dirname "$log")"
  cd "$(main_checkout)" || exit 1
  exec env -i "HOME=$HOME" "USER=${USER:-}" \
    "PATH=$(dirname "$lf_bin"):$HOME/.local/bin:$HOME/.cargo/bin:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin" \
    "LF_HOME=$lf_home" "LOOPFLOW_SRC=$src" "TMPDIR=${TMPDIR:-/tmp}" "LANG=${LANG:-en_US.UTF-8}" \
    "$lf_bin" --batch skill disk-cleanup >>"${log%.log}-skill.log" 2>&1
}

case "${1:-run}" in
  install) install_entry; exit $? ;;
  uninstall)
    without_entry | crontab - || exit 1
    printf 'Removed the disk-cleanup crontab entry\n'
    exit 0 ;;
  skill) run_skill ;;
  run) ;;
  *) sed -n '2,27p' "$0" >&2; exit 2 ;;
esac

# --------------------------------------------------------------------------
# run

mkdir -p "$(dirname "$log")" "$(dirname "$disk_log")"
# Keep the last ~10 MB of history.
if [ -f "$log" ] && [ "$(stat -f %z "$log")" -gt 10485760 ]; then
  tail -c 5242880 "$log" >"$log.tmp" && mv "$log.tmp" "$log"
fi
exec > >(tee -a "$log") 2>&1

lock="${TMPDIR:-/tmp}/loopflow-disk-cleanup.lock"
if ! mkdir "$lock" 2>/dev/null; then
  printf '%s skipped: another cleanup holds %s (if abandoned, remove it after checking for a live run)\n' "$(date '+%F %T')" "$lock"
  exit 0
fi
work="$(mktemp -d)"
trap 'rm -rf "$lock" "$work"' EXIT

free_kib() { df -k "$HOME" | awk 'NR==2 {print $4}'; }
human() { awk -v k="$1" 'BEGIN { split("K M G T", u); i=1; while (k>=1024 && i<4) { k/=1024; i++ } printf "%.1f%s", k, u[i] }'; }
note() { printf '%s %s\n' "$(date '+%F %T')" "$*"; }
step() {
  local name="$1" before after status
  shift
  inspect_open_files || exit 1
  before="$(free_kib)"
  note "== $name"
  "$@" 2>&1 | tee -a "$disk_log" | sed 's/^/   /'
  status=$?
  [ "$status" -eq 0 ] || note "FAILED: $name (exit $status)"
  after="$(free_kib)"
  note "   $name freed $(human $((after > before ? after - before : 0)))"
}

# step appends this receipt to disk.log: remove <category> <path> <reason>
remove() {
  local kib
  kib="$(du -sk "$2" 2>/dev/null | awk '{print $1}')"
  rm -rf "$2" || return
  printf '%s removed %s %s %s (%s)\n' "$(date -u '+%FT%TZ')" "$1" "$(human "${kib:-0}")" "$2" "$3"
}

# Every open path (cwd, mapped files, open files) and all their ancestors, read
# before each step: a path is held when it, or anything under it, is open.
held="$work/held"
inspect_open_files() {
  if ! lsof -nP -Fn 2>/dev/null >"$work/lsof" || ! grep -q '^n/' "$work/lsof"; then
    echo "open-file inspection failed; stopping cleanup" >&2
    return 1
  fi
  sed -n 's/^n\(\/.*\)/\1/p' "$work/lsof" | awk '{
    p = $0; print p
    while (sub(/\/[^\/]*$/, "", p) && p != "") print p
  }' | sort -u >"$held"
}

# Print the paths on stdin that no process holds.
idle() {
  awk 'NR == FNR { h[$0] = 1; next } !($0 in h)' "$held" -
}

# Whether nothing under the path was modified within the minutes given.
untouched() {
  local recent
  if ! recent="$(find "$1" -mmin -"$2" -print -quit 2>/dev/null)"; then
    echo "kept $1 (modification inspection failed)" >&2
    return 1
  fi
  [ -z "$recent" ]
}

# --------------------------------------------------------------------------
# Home

home_units() {
  local unit
  for unit in "$lf_home"/runs/*/*/; do [ -d "$unit" ] && printf 'captures\t%s\n' "${unit%/}"; done
  for unit in "$lf_home"/logs "$lf_home"/log; do
    [ -d "$unit" ] && find "$unit" -type f | sed 's/^/logs\t/'
  done
  # Draft builds no longer make store copies; reclaim the ones they left.
  for unit in "$lf_home"/copies/*/ "$lf_home"/copies/.tmp*/; do
    [ -d "$unit" ] && printf 'copies\t%s\n' "${unit%/}"
  done
  # Backups by database, newest first; all but the first of each go, each
  # with its -wal/-shm sidecars.
  find "$lf_home" -maxdepth 1 -type f \( -name '*.db.backup*' -o -name '*.db.bak*' -o -name '*.db.pre*' \) \
    ! -name '*-wal' ! -name '*-shm' ! -name '*.lock' -exec stat -f '%m%t%N' {} + 2>/dev/null |
    sort -rn | cut -f2 | awk '{ db = $0; sub(/\.db\..*/, ".db", db); if (seen[db]++) print "backups\t" $0 }'
}

home() {
  local category path sidecar keep count=0
  home_units >"$work/home"
  cut -f2 "$work/home" | idle >"$work/home-idle"
  while IFS=$'\t' read -r category path; do
    [ "$path" != "$disk_log" ] || continue
    grep -qxF -- "$path" "$work/home-idle" || continue
    untouched "$path" $((home_keep_days * 1440)) || continue
    keep=false
    for sidecar in "$path-wal" "$path-shm"; do
      [ -e "$sidecar" ] || continue
      if grep -qxF -- "$sidecar" "$held" || ! untouched "$sidecar" $((home_keep_days * 1440)); then
        keep=true
      fi
    done
    [ "$keep" = false ] || continue
    remove "$category" "$path" "older than $home_keep_days days" || continue
    for sidecar in "$path-wal" "$path-shm"; do
      [ ! -e "$sidecar" ] || remove "$category" "$sidecar" "older than $home_keep_days days"
    done
    count=$((count + 1))
  done <"$work/home"
  echo "removed $count Home units untouched for $home_keep_days days"
}

# --------------------------------------------------------------------------
# Repositories

# Ignored, untracked output in one checkout that no process holds and nothing
# wrote to for build_idle_minutes: idle_builds <checkout>
idle_builds() {
  local checkout="$1" path rel tracked
  inspect_open_files || return 1
  if grep -qxF -- "$checkout" "$held"; then
    echo "kept $checkout (held open)"
    return
  fi
  find "$checkout" -maxdepth 3 -type d \( -name target -o -name node_modules -o -name .venv \
    -o -name .build -o -name DerivedData \) -prune 2>/dev/null | sort >"$work/builds"
  while IFS= read -r path; do
    rel="${path#"$checkout"/}"
    git -C "$checkout" check-ignore -q -- "$rel" || continue
    tracked="$(git -C "$checkout" ls-files -- "$rel")" || continue
    [ -z "$tracked" ] || continue
    if ! untouched "$path" "$build_idle_minutes"; then
      echo "kept $path (written in the last 4 hours)"
    else
      remove build "$path" "idle 4 hours in $(basename "$checkout")"
    fi
  done <"$work/builds"
}

# Linked worktrees of a main checkout that sit directly under ~/src.
linked_worktrees() {
  local git_dir wt gitdir
  git_dir="$(cd "$1/.git" && pwd -P)" || return
  for wt in "$src"/*/; do
    [ -f "$wt/.git" ] || continue
    gitdir="$(sed -n 's/^gitdir: //p' "$wt/.git")"
    [ "$(cd "${gitdir%/worktrees/*}" 2>/dev/null && pwd -P)" = "$git_dir" ] && printf '%s\n' "${wt%/}"
  done
}

repository() {
  local repo="$1" wt
  (
    cd "$repo" || exit
    lf wt prune || echo "lf wt prune failed"
    lf wt prune --builds --maximum-roots 100000 --maximum-bytes 1099511627776 ||
      echo "lf wt prune --builds failed"
    lf wt prune --branches || echo "lf wt prune --branches failed"
  )
  idle_builds "$repo"
  linked_worktrees "$repo" >"$work/worktrees"
  while IFS= read -r wt; do
    idle_builds "$wt"
  done <"$work/worktrees"
}

# --------------------------------------------------------------------------
# Machine caches

uv_cache() {
  command -v uv >/dev/null || { echo "uv not installed"; return; }
  local dir kib
  dir="$(uv cache dir)"
  kib="$(du -sk "$dir" 2>/dev/null | awk '{print $1}')"
  echo "uv cache $(human "${kib:-0}") at $dir"
  if [ "${kib:-0}" -gt $((uv_clean_over_gib * 1024 * 1024)) ] && ! pgrep -x uv >/dev/null; then
    UV_LOCK_TIMEOUT=120 uv cache clean
  else
    UV_LOCK_TIMEOUT=120 uv cache prune
  fi
}

simulators() {
  command -v xcrun >/dev/null || { echo "Xcode not installed"; return; }
  xcrun simctl delete unavailable
}

library_caches() {
  # Whole entries only. Deleting old files or empty directories inside a live
  # cache leaves it half-written: SwiftPM's git mirrors lost their empty refs/
  # and old packs, and the release's Mac build then failed to resolve packages.
  local entry count=0
  for entry in "$HOME/Library/Caches"/*; do
    [ -e "$entry" ] || continue
    untouched "$entry" $((cache_idle_days * 1440)) || continue
    grep -qxF -- "$entry" "$held" && { echo "kept $entry (held open)"; continue; }
    remove cache "$entry" "untouched for $cache_idle_days days" && count=$((count + 1))
  done
  echo "removed $count cache entries untouched for $cache_idle_days days"
}

trash() {
  if ! ls "$HOME/.Trash" >/dev/null 2>&1; then
    echo "$HOME/.Trash unreadable: grant Full Disk Access to /bin/bash to clean it"
    return
  fi
  local entry count=0
  while IFS= read -r entry; do
    untouched "$entry" $((trash_idle_days * 1440)) || continue
    grep -qxF -- "$entry" "$held" && { echo "kept $entry (held open)"; continue; }
    remove trash "$entry" "untouched for $trash_idle_days days" && count=$((count + 1))
  done < <(find "$HOME/.Trash" -mindepth 1 -maxdepth 1 -print)
  echo "removed $count items trashed over $trash_idle_days days ago"
}

docker_image() {
  local raw="$HOME/Library/Containers/com.docker.docker/Data/vms/0/data/Docker.raw"
  [ -f "$raw" ] || { echo "no Docker disk image"; return; }
  echo "Docker.raw holds $(du -sh "$raw" | awk '{print $1}') on disk; reclaim with Docker Desktop's disk resize, not deletion"
}

start="$(free_kib)"
note "disk cleanup on $(hostname -s): $(human "$start") free"
step "lf monitor prune" lf monitor prune
step "Home" home
for repo in "$src"/*/; do
  [ -d "$repo/.git" ] && step "repository $(basename "$repo")" repository "${repo%/}"
done
step "uv cache" uv_cache
step "simulators" simulators
step "Library caches" library_caches
step "trash" trash
step "docker" docker_image
end="$(free_kib)"
note "done: $(human "$end") free, freed $(human $((end > start ? end - start : 0)))"
