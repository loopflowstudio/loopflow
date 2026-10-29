# LOO-682 verify (2026-10-28)

## Exists
- PR #1764 merged as 334c05cd0; ~/src/loopflow is clean on main (now 1a60c40fc).
- Source (#1764): on macOS, `scripts/bootstrap-cron-host.sh` loads two gui-domain LaunchAgents, removes only the Wave's `# loopflow:<wave>:` crontab lines, then kickstarts telemetry-daily and waits for its exit code.
- Fix-forward on this branch (new PR, accepted at verify pass 1): the crontab removal now runs before either agent loads; `test_cron_bootstrap_loads_no_macos_agent_until_the_wave_leaves_crontab` fails on the #1764 order. CRON_HOST.md says so.

## Machine state (re-inspected at realign, 2026-10-28)
- Both jobs still run from the old crontab lines (telemetry 09:00, release-run 10:00); no `loopflow.cron.infrastructure.*` agent is loaded or present in ~/Library/LaunchAgents.
- The headless bootstrap rerun passed installed lf 0.15.0, placement, tool path, provider authority and the publisher preflight, loaded both agents, then blocked over 40 minutes in `crontab -` (apparently an unanswered macOS permission prompt). It was stopped (exit 143) with the crontab unchanged; both agents were booted out and their plists moved to /tmp/loo-682/ with bootstrap.log and crontab.before, so release-run is never scheduled twice. Telemetry never ran.

## Remains
- Installed acceptance: from a logged-in Terminal on this Mac, run `scripts/bootstrap-cron-host.sh infrastructure` in ~/src/loopflow and approve the crontab prompt. Confirm two loaded agents, the Wave's crontab lines gone with other entries kept, and a green kickstarted telemetry-daily run. Never start release-run by hand. Two real calendar firings remain LOO-645's evidence.
- Finding: an lf-launched shell bakes its PATH (Loopflow Dev.app) into the regenerated wrappers; lf itself stays pinned to the install gate. Running from a plain Terminal avoids it.

Check: `uv run pytest python/tests/test_release_automation.py -k cron` — 8 passed (2026-10-28); gate/PR remain.

## Verify pass 1 decision (2026-10-28): iterate
- Evidence: a one-shot gui-domain LaunchAgent probe (RunAtLoad, no schedule, booted out after) in ~/src/loopflow got exit 0 from `doppler secrets --only-names` and from the Claude Keychain item lookup (read only; telemetry's failure was a write). The LaunchAgent design reaches the keyring cron could not.
- Removing the cron lines first loses nothing (they already fail on Keychain); a headless `crontab -` hang now leaves at most the old broken schedule.
