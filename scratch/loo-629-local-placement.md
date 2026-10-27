# LOO-629: cron bootstrap on the local default Machine

Status (2026-10-27): script repair and cutover reorder committed; repair PR
#1732 open; host cutover pending after landing.

## Done in this branch
- `scripts/bootstrap-cron-host.sh`: `.wave.machine == null` (no Wave Machine
  default → runs on the reader) is accepted; a Wave placed on another Machine
  still exits 1 before touching the crontab.
- Every check and both cron lines run the installed entry gate
  `~/.lf-machine/install/gates/1/lf`, not PATH's first `lf` (Jack's shells put
  `Loopflow Dev.app` ahead of it in some contexts).
- Cron env assignments are `%q`-quoted: Jack's PATH contains
  `Loopflow Dev.app/...` (a space), which split the old unquoted `env -i PATH=…`.
  The cron lines are rendered from the same `minimal_env` the checks run in.
- Tests: behavioral runs of the real script with fake host tools for implicit
  local, explicit local and foreign placement; executes the installed
  release-run crontab line under `sh` with an empty env.

## Concept review (2026-10-27): decisions
Placement stays one concept: the Wave's Machine default, where null means "the
Machine reading it". No new flag or explicit-local requirement.

- Cutover order (done): write the crontab, confirm both `# loopflow:<wave>:`
  lines with `crontab -l`, then boot out and remove the launchd plists, then
  run telemetry. A failed `crontab -` leaves launchd in place (tested).
- Accepted counterexample, no change: with null placement, two Machines that
  both bootstrap would both schedule releases. Jack's Task accepted implicit
  local placement; pinning elsewhere uses `lf machine use` (CRON_HOST.md).
- Gate path `~/.lf-machine/install/gates/1/lf` is what lf re-execs to
  (`installation.rs` `GATE_DIRECTORY`); a gate version change fails loudly and
  a rerun repairs it.
- Second pass (after the reorder and fixture commits): nothing new to change;
  CRON_HOST.md matches the script; no open product choice.

## Remaining live acceptance
Land repair PR #1732 (#1711 merged; it is history); main's script still
exits 1 on the null placement until it lands.

1. Dependency resolved 2026-10-27: v0.14.18 published 14:21Z and the gate
   reports `lf 0.14.18`; `lf wave status infrastructure --json` now parses and
   gives `.wave.machine == null`.
2. From `~/src/loopflow` on main: `scripts/bootstrap-cron-host.sh infrastructure`.
   Expect: launchd `loopflow.cron.infrastructure.{telemetry-daily,release-run}`
   removed, two crontab lines (09:00/10:00) pinned to the gate, telemetry run green.
   Crontab still empty and both launchd plists still present as of this pass.
3. Release path: main has 5 commits after v0.14.18 (cut at 058b4d0b), so the first
   `release-run` would publish v0.14.19, not take the no-change path. Retain
   the 2026-10-28 10:00 firing's `.lf/logs/release-run.log` as the scheduled
   proof unless Jack directs a manual run.

Check: `uv run pytest python/tests/test_release_automation.py -q` → 14 passed.

## Demo (2026-10-27, headless, standing in for Jack)
Experience: the placement check against this Mac's real Wave state, gate `lf 0.14.18`.
- Live `wave status infrastructure --json` → `{"machine":null}`; crontab empty;
  both `loopflow.cron.infrastructure.*` launchd plists present.
- Old filter `.wave.machine.id` on that output: prints `null`, exit 1 — the
  failure in `/tmp/loo629-bootstrap-live.log` (stops at "placed Machine").
- Branch filter: empty id, exit 0 → "runs on this Machine" path.
- Explicit and foreign placement, quoted cron env, cutover order: covered by
  behavioral script tests, `uv run pytest python/tests/test_release_automation.py -q` → 14 passed.
- Full bootstrap not run here: the script refuses non-main checkouts by design,
  so the real cutover runs from `~/src/loopflow` after #1732 lands (step 2 above).
Decision: advance. Next proof: land #1732, run step 2, retain the 10:00 release-run log.
