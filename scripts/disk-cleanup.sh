#!/usr/bin/env bash
# Hourly disk cleanup for Jack's machines (the laptop and mini-heart).
#
# Runs `lf disk clean`, remote-branch cleanup once LOO-539 ships it, then the
# machine caches Jack approved: uv cache, unavailable simulators, idle
# ~/Library/Caches files, idle cargo target/ dirs outside lf worktrees and old
# Trash. Each step logs what it removed and how much disk it freed.
#
# Usage:
#   scripts/disk-cleanup.sh              run once
#   scripts/disk-cleanup.sh install      run hourly via launchd (from the main checkout)
#   scripts/disk-cleanup.sh uninstall
#
# Log: ~/Library/Logs/loopflow-disk-cleanup.log
set -uo pipefail

label="com.loopflow.disk-cleanup"
plist="$HOME/Library/LaunchAgents/$label.plist"
log="$HOME/Library/Logs/loopflow-disk-cleanup.log"
src="${LOOPFLOW_SRC:-$HOME/src}"
uv_clean_over_gib=10
cache_idle_days=7
trash_idle_days=30

# --------------------------------------------------------------------------
# install / uninstall

install_agent() {
  local repo main script
  repo="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
  main="$(dirname "$(git -C "$repo" rev-parse --path-format=absolute --git-common-dir)")"
  script="$main/scripts/disk-cleanup.sh"
  if [ ! -f "$script" ]; then
    printf 'install from a main checkout that has %s (land LOO-538 first)\n' "$script" >&2
    exit 1
  fi
  mkdir -p "$(dirname "$plist")" "$(dirname "$log")"
  cat >"$plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key><string>$label</string>
  <key>ProgramArguments</key>
  <array><string>/bin/bash</string><string>$script</string></array>
  <key>StartInterval</key><integer>3600</integer>
  <key>ProcessType</key><string>Background</string>
  <key>LowPriorityIO</key><true/>
  <key>Nice</key><integer>10</integer>
  <key>EnvironmentVariables</key>
  <dict>
    <key>PATH</key><string>$HOME/.local/bin:$HOME/.cargo/bin:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin</string>
    <key>LOOPFLOW_SRC</key><string>$src</string>
  </dict>
  <key>StandardOutPath</key><string>$log</string>
  <key>StandardErrorPath</key><string>$log</string>
</dict>
</plist>
EOF
  launchctl bootout "gui/$(id -u)/$label" 2>/dev/null || true
  launchctl bootstrap "gui/$(id -u)" "$plist"
  printf 'Installed %s: hourly %s, logging to %s\n' "$label" "$script" "$log"
}

case "${1:-run}" in
  install) install_agent; exit 0 ;;
  uninstall)
    launchctl bootout "gui/$(id -u)/$label" 2>/dev/null || true
    rm -f "$plist"
    printf 'Removed %s\n' "$label"
    exit 0 ;;
  run) ;;
  *) sed -n '2,14p' "$0" >&2; exit 2 ;;
esac

# --------------------------------------------------------------------------
# run

mkdir -p "$(dirname "$log")"
# Keep the last ~10 MB of history.
if [ -f "$log" ] && [ "$(stat -f %z "$log")" -gt 10485760 ]; then
  tail -c 5242880 "$log" >"$log.tmp" && mv "$log.tmp" "$log"
fi
# Under launchd stdout is already the log; by hand, mirror to it.
if [ -t 1 ]; then exec > >(tee -a "$log") 2>&1; fi

lock="${TMPDIR:-/tmp}/loopflow-disk-cleanup.lock"
if ! mkdir "$lock" 2>/dev/null; then
  if [ -n "$(find "$lock" -maxdepth 0 -mmin +120 2>/dev/null)" ]; then
    rm -rf "$lock" && mkdir "$lock"
  else
    printf '%s skipped: another cleanup holds %s\n' "$(date '+%F %T')" "$lock"
    exit 0
  fi
fi
trap 'rm -rf "$lock"' EXIT

free_kib() { df -k "$HOME" | awk 'NR==2 {print $4}'; }
human() { awk -v k="$1" 'BEGIN { split("K M G T", u); i=1; while (k>=1024 && i<4) { k/=1024; i++ } printf "%.1f%s", k, u[i] }'; }
note() { printf '%s %s\n' "$(date '+%F %T')" "$*"; }
step() {
  local name="$1" before after
  shift
  before="$(free_kib)"
  note "== $name"
  "$@" 2>&1 | sed 's/^/   /'
  after="$(free_kib)"
  note "   $name freed $(human $((after > before ? after - before : 0)))"
}

# Directories any process holds open (cwd, mapped files, open files), read once.
held_paths="$(lsof -nP -Fn 2>/dev/null | sed -n 's/^n\(\/.*\)/\1/p' | sort -u)"
is_held() { printf '%s\n' "$held_paths" | grep -qF -- "$1" ; }

lf_clean() {
  if ! lf disk --help >/dev/null 2>&1; then
    echo "installed lf has no 'disk' command yet; skipped"
    return
  fi
  lf disk clean
}

remote_branches() {
  # LOO-539: remote-branch cleanup. It plans `lf wt prune --branches`, run per
  # repository; this runs it once the installed lf has the flag.
  if ! lf wt prune --help 2>/dev/null | grep -q -- '--branches'; then
    echo "lf wt prune --branches not available yet (LOO-539); skipped"
    return
  fi
  local repo
  for repo in "$src"/*/; do
    [ -d "$repo/.git" ] || continue
    echo "${repo%/}"
    (cd "$repo" && lf wt prune --branches)
  done
}

uv_cache() {
  command -v uv >/dev/null || { echo "uv not installed"; return; }
  local dir kib
  dir="$(uv cache dir)"
  kib="$(du -sk "$dir" 2>/dev/null | awk '{print $1}')"
  echo "uv cache $(human "${kib:-0}") at $dir"
  if [ "${kib:-0}" -gt $((uv_clean_over_gib * 1024 * 1024)) ] && ! pgrep -x uv >/dev/null; then
    uv cache clean
  else
    uv cache prune
  fi
}

simulators() {
  command -v xcrun >/dev/null || { echo "Xcode not installed"; return; }
  xcrun simctl delete unavailable
}

library_caches() {
  local count
  count="$(find "$HOME/Library/Caches" -type f -mtime +"$cache_idle_days" 2>/dev/null | wc -l | tr -d ' ')"
  find "$HOME/Library/Caches" -type f -mtime +"$cache_idle_days" -delete 2>/dev/null
  find "$HOME/Library/Caches" -mindepth 1 -type d -empty -delete 2>/dev/null
  echo "removed $count files untouched for $cache_idle_days days"
}

cargo_targets() {
  # Main checkouts and plain projects only: lf owns its linked worktrees'
  # output (`lf disk clean`), and those have a .git file rather than a directory.
  local target top kib
  while IFS= read -r target; do
    [ -f "$target/CACHEDIR.TAG" ] || continue
    top="$(git -C "$(dirname "$target")" rev-parse --show-toplevel 2>/dev/null)"
    [ -n "$top" ] && [ -f "$top/.git" ] && continue
    if is_held "$target"; then
      echo "kept $target (a process is using it)"
      continue
    fi
    if [ -n "$(find "$target" -mmin -1440 -print -quit 2>/dev/null)" ]; then
      echo "kept $target (written in the last day)"
      continue
    fi
    kib="$(du -sk "$target" | awk '{print $1}')"
    rm -rf "$target" && echo "removed $target $(human "$kib")"
  done < <(find "$src" -maxdepth 3 -type d -name target -prune 2>/dev/null)
}

trash() {
  if ! ls "$HOME/.Trash" >/dev/null 2>&1; then
    echo "~/.Trash unreadable: grant Full Disk Access to /bin/bash to clean it"
    return
  fi
  local count
  count="$(find "$HOME/.Trash" -mindepth 1 -maxdepth 1 -mtime +"$trash_idle_days" | wc -l | tr -d ' ')"
  find "$HOME/.Trash" -mindepth 1 -maxdepth 1 -mtime +"$trash_idle_days" -exec rm -rf {} + 2>/dev/null
  echo "removed $count items trashed over $trash_idle_days days ago"
}

docker_image() {
  local raw="$HOME/Library/Containers/com.docker.docker/Data/vms/0/data/Docker.raw"
  [ -f "$raw" ] || { echo "no Docker disk image"; return; }
  echo "Docker.raw holds $(du -sh "$raw" | awk '{print $1}') on disk; reclaim with Docker Desktop's disk resize, not deletion"
}

start="$(free_kib)"
note "disk cleanup on $(hostname -s): $(human "$start") free"
step "lf disk clean" lf_clean
step "remote branches" remote_branches
step "uv cache" uv_cache
step "simulators" simulators
step "Library caches" library_caches
step "cargo targets" cargo_targets
step "trash" trash
step "docker" docker_image
end="$(free_kib)"
note "done: $(human "$end") free, freed $(human $((end > start ? end - start : 0)))"
