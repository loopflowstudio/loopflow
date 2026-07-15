#!/usr/bin/env bash
#
# Bootstrap a maintained lf cron host.
#
# Probes reachability + auth (bounded `lf ssh`), verifies lf and Doppler are
# present, syncs the wave's repo-owned schedules onto the host, and lists the
# result. Idempotent and secret-free — re-run any time to reconcile.
#
# Usage: scripts/bootstrap-cron-host.sh <ssh-host-or-alias> [wave]
#   scripts/bootstrap-cron-host.sh mini-heart infrastructure
#
# Secrets are configured out of band via `doppler setup` on the host; this
# script never reads, prints, or forwards a value.
set -euo pipefail

host="${1:?usage: bootstrap-cron-host.sh <ssh-host-or-alias> [wave]}"
wave="${2:-infrastructure}"

step() { printf '\n== %s ==\n' "$1"; }

step "reachability + auth ($host)"
# Bounded lf ssh: an unreachable host fails in ~10s instead of hanging.
lf ssh "$host" -- lf --version

step "host prerequisites"
lf ssh "$host" -- doppler --version

step "sync repo-owned schedules (wave: $wave)"
lf ssh "$host" -- lf cron sync --wave "$wave"

step "installed schedules"
lf ssh "$host" -- lf cron list

printf '\nbootstrap complete: %s runs wave %s schedules via launchd\n' "$host" "$wave"
