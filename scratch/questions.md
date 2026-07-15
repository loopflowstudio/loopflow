# W2-78 open questions / assumptions

- **Daily flow choice.** Assumed `telemetry-daily` (which runs `op: doctor`, a
  read-only health gate that exits non-zero on failure) is the wave's daily health
  cron. GOAL prose asks the daily check to cover "architecture drift, local
  development friction, CI, release cadence, spend, and host health" — doctor is
  the closest existing gate. If a richer daily flow is wanted, change the one line
  in `wave/infrastructure/GOAL.md` and re-run `lf cron sync`.

- **Schedule time.** Assumed `09:00` daily (`0 0 9 * * *`), matching the nightly CI
  time in `release/SCHEDULE.md`. Applied host-local on launchd (see CRON_HOST.md
  timezone note). Reversible: edit GOAL.md, re-sync.

- **External blocker (not an assumption — evidence).** mini-heart is unreachable
  from this worktree (`lf ssh mini-heart` and the raw tailnet IP both time out; no
  `~/.ssh/config` alias). The provisioning half — running the bootstrap against the
  host and proving a nightly run *from* the host — is blocked until it answers on
  the tailnet. The repo-committable half (the `lf cron sync` feature, declared
  schedule, host doc, bootstrap script) is done and verified locally.

- **Homes registry.** The host is named in `release/CRON_HOST.md` (greppable,
  agent-readable) rather than a machine `.lf/homes.yaml`, because nothing consumes
  such a file yet and dead config violates the very "committed fact" principle. A
  machine-readable home registry + `lf` discovery is a separate follow-up.
