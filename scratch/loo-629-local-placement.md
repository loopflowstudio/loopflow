# LOO-629: cron bootstrap on the local default Machine

Status (2026-10-27): script repair committed (296590fc74); live host cutover pending.

## Done in this branch
- `scripts/bootstrap-cron-host.sh`: `.wave.machine == null` (no Wave Machine
  default → runs on the reader) is accepted; a Wave placed on another Machine
  still exits 1 before touching the crontab.
- Every check and both cron lines run the installed entry gate
  `~/.lf-machine/install/gates/1/lf`, not PATH's first `lf` (Jack's shells put
  `Loopflow Dev.app` ahead of it in some contexts).
- Cron env assignments are `%q`-quoted: Jack's PATH contains
  `Loopflow Dev.app/...` (a space), which split the old unquoted `env -i PATH=…`.
- Tests: behavioral runs of the real script with fake host tools for implicit
  local, explicit local and foreign placement; executes the installed
  release-run crontab line under `sh` with an empty env.

## Remaining live acceptance (after this PR lands)
1. Dependency: installed gate is v0.14.17, whose `lf wave status --json` fails
   (`missing field interactive`, fixed by #1722 in v0.14.18). v0.14.18 was in
   `publish_release.py prepare` at 07:15; do not start another release.
   Once published, the refresh LaunchAgent (or `lf self install`) promotes it.
2. From `~/src/loopflow` on main: `scripts/bootstrap-cron-host.sh infrastructure`.
   Expect: launchd `loopflow.cron.infrastructure.{telemetry-daily,release-run}`
   removed, two crontab lines (09:00/10:00) pinned to the gate, telemetry run green.
   Live crontab was empty before (no unrelated entries to preserve).
3. Release path: main already has commits after v0.14.18, so the first
   `release-run` would publish v0.14.19, not take the no-change path. Retain
   the 2026-10-28 10:00 firing's `.lf/logs/release-run.log` as the scheduled
   proof unless Jack directs a manual run.

Check: `uv run pytest python/tests/test_release_automation.py -q` → 13 passed.
