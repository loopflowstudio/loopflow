# LOO-629: Scheduled release-run cron fails every run

Status: implemented 2026-10-27 (steps 1–3); step 4 is the post-merge cutover. Jack approved shipping PR #1711 from
demo on 2026-10-27, before main's #1708 removed the code that PR patches.

## Problem

Jack Heart (2026-10-26): the infrastructure Wave's scheduled `release-run`
(`0 0 10 * * *`, home_39860354aaca640c2ccb50bf6ca609d8) shows `failed` while
releases are being cut by hand. The next scheduled run should release, or
correctly report that there is nothing to release. Prove it with the command the
cron runs. Do not cut a release as part of this Task.

## Findings (2026-10-27)

- **Cause, confirmed in `~/src/loopflow/.lf/logs/cron.infrastructure.release-run.log`.**
  The 2026-10-21 cron attempt saved a v0.14.13 selection and failed during
  publish. v0.14.13–v0.14.17 were then published by hand, but each later
  scheduled attempt resumed the saved selection. It re-verified v0.14.13's
  source, and `check_migrations.py` rejected it:
  `0.14.15.001_release.sql exists on origin/main but is missing from this branch`.
  The unsettled opportunity carried into every receipt (10-23, 10-26).
- **PR #1711's fix is obsolete on main.** #1708 (LOO-610) deleted
  `ops/cron/{accounting,history,published,calendar}.rs`, `release_run_with_cron`
  and `scheduled_release_tests.rs`. Main's `release_run_single` has no saved
  selection: it reads origin/main and the latest tag and either releases or
  returns `NoChanges`. The failure class is gone from the code. The PR conflicts
  (`DIRTY`): release.rs content, plus modify/delete on the test file.
- **The live schedule is still the old mechanism.** The Machine runs launchd
  agents `loopflow.cron.infrastructure.{release-run,telemetry-daily}`
  (`~/Library/LaunchAgents/*.plist`), invoking
  `~/.lf-machine/install/gates/1/lf wave cron run --scheduled …`. The installed
  lf is `0.14.17+fd29cbcde`, which predates #1708, so tomorrow's run still takes
  the saved-selection path and fails the same way. The crontab has no
  `loopflow:` lines.
- **Main's cutover doesn't retire the old schedule.** #1708 replaced the
  scheduler with crontab lines written by `scripts/bootstrap-cron-host.sh`, but
  that script never removes the launchd agents. After bootstrap the Machine
  would have two schedulers. Once an lf containing #1708 is installed, the
  launchd agents call a removed command (`lf wave cron run`) and fail every day,
  and `lf wave cron list`, the only view that reported them, is gone too.
- **Installed lf can already run the crontab line.** With 0.14.17,
  `lf release run` resolves to `repo release run` through shorthand, and the
  non-cron path passes no saved selection. Bootstrap therefore works today and
  does not need to wait for the next release.

## Chosen approach

1. **Drop the obsolete diff.** Rebase the branch onto main and resolve both
   conflicts to main's version. Nothing from the old release.rs/test change
   survives, because its subject no longer exists.
2. **Retire the launchd scheduler in bootstrap.** In
   `scripts/bootstrap-cron-host.sh`, in the "install release schedules" step,
   before writing the crontab: `launchctl bootout gui/$(id -u)/<label>` and
   delete `~/Library/LaunchAgents/loopflow.cron.$wave.*.plist`, printing each
   label it removes. If none exist, skip quietly; this runs on every host. It
   runs after the preflight checks, so a failed check still changes nothing.
   This is the consumer cutover #1708 needed for its deletion, not a
   compatibility path. Nothing in main writes those plists any more
   (`git grep 'loopflow.cron\.'` finds nothing).
3. **Document it** in `release/CRON_HOST.md`: bootstrap replaces any earlier
   launchd `loopflow.cron.<wave>.*` agents, and `crontab -l` plus
   `launchctl list | grep loopflow.cron` show which scheduler is live.
4. **Cut over the live Machine.** This is an operational step after merge, run
   from `~/src/loopflow` on main: `scripts/bootstrap-cron-host.sh infrastructure`.
   It runs telemetry-daily as its configured-path check and does not run
   release-run, so no release is cut.

### Alternatives rejected

- **Land #1711 as approved by porting it forward.** There is nothing to port
  onto; the accounting module is deleted. Reviving it would undo #1708.
- **Install an lf containing #1708 and let the launchd job fail differently.**
  It would still be red daily and no longer visible anywhere.
- **Hand-edit the live Machine without a script change.** That would fix this
  host once, but every other host bootstrapped from an older lf would keep the
  duplicate scheduler.

## Remaining

- **CI on head `99dfe3694f` is in progress** (2026-10-27): rust-lint, migration,
  architecture and website checks pass; rust-build, python-test, swift and e2e pending.
- Step 4 (live cutover) and the next scheduled run's log, after merge.

## Demo

On the cron host after merge:

```bash
scripts/bootstrap-cron-host.sh infrastructure
# == install release schedules ==
# removed launchd job loopflow.cron.infrastructure.release-run
# removed launchd job loopflow.cron.infrastructure.telemetry-daily
# 0 10 * * * cd … lf --wave infrastructure --batch graph release-run … # loopflow:infrastructure:release-run
launchctl list | grep loopflow.cron     # nothing
```

The next 10:00 run appends to `.lf/logs/release-run.log`. It either publishes
the next patch from main or reports that nothing changed since the latest tag,
and exits 0.

## Scope

- **User-visible outcome:** Jack sees one scheduler. The daily release job
  either releases or reports no changes, and its exit status is the job's.
- **Source of truth:** the crontab line, plus the release command's exit status
  and public read-back. Launchd plists stop being a scheduler.
- **Affected surfaces:** `scripts/bootstrap-cron-host.sh`, `release/CRON_HOST.md`,
  `python/tests/test_release_automation.py` (if it asserts bootstrap content).
- **Absent/error states:** no plists → no-op. `launchctl bootout` of an
  unloaded label → ignore and still delete the file. Preflight failure → nothing
  removed.
- **Exclusions:** cutting a release; `com.loopflow.lfd` and other non-cron
  launchd agents (the lfd deletion is separate, Jack 2026-09-27); Linux hosts
  (no launchd, the step is skipped).

## Delete — do not maintain

- Live `~/Library/LaunchAgents/loopflow.cron.infrastructure.*.plist`, removed by
  bootstrap.

## Forbidden outcomes

- launchd and crontab both scheduling `release-run`.
- Reintroducing cron accounting, saved selections or `lf wave cron` to settle
  the old v0.14.13 opportunity. Its record goes with the deleted module.

## Done when

- The branch carries only the bootstrap/doc change on top of main, and CI is
  green.
- Gate (headless): `bash -n scripts/bootstrap-cron-host.sh` passes, plus the
  focused check
  `uv run pytest python/tests/test_release_automation.py -q` (extend it to
  assert that bootstrap removes `loopflow.cron.$wave.*` agents before writing
  the crontab).
- After merge, on the cron host: bootstrap prints the removed labels,
  `launchctl list | grep loopflow.cron` is empty, and `crontab -l | grep
  loopflow:infrastructure` shows both jobs. The next scheduled `release-run`
  exits 0 with a publication or "no changes". The Task completes on that run's
  log, not on merge.

## Check results

- Gate 2026-10-27 on `99dfe3694f`: `bash -n scripts/bootstrap-cron-host.sh` ok; `uv run pytest python/tests/test_release_automation.py -q` 10 passed; shellcheck not installed and not in CI; PR CI in progress (deferred to CI). PR title and body already match the diff.
- `bash -n` + `uv run pytest python/tests/test_release_automation.py -q` 10 passed, fake-HOME cleanup run ok; code unchanged since, origin/main unchanged since base `4a85fb3a5a`; live Machine 2026-10-27 still has both launchd cron agents (last exit 1) and no `loopflow:` crontab lines; CI deferred to the PR.
- Concept review 2026-10-27: no change. One scheduler (crontab line), one owner (`bootstrap-cron-host.sh`), one record (the job's log and exit status). Retiring every `loopflow.cron.$wave.*` agent, not only the two replaced jobs, is implied: their command no longer exists on main. Whether Jack's #1711 approval covers the new diff is a delivery question (see Remaining), not a product choice.
