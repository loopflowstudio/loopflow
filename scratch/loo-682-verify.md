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

## Concept review (2026-10-28): nothing to change
- One action, few concepts: run the bootstrap once on the placed Machine; each job is one generated wrapper around a plain `lf --wave … --batch graph <job>` command, run by the platform scheduler (gui-domain LaunchAgent on macOS, crontab on Linux). Matches the accepted "Loopflow keeps no scheduler" contract in docs/ops.md and docs/work.md.
- Failure/recovery path holds: a stopped bootstrap leaves at most the old (already Keychain-broken) cron lines and no agent, so release-run never has two schedulers; rerunning converges.
- Implied follow-up, not now: on macOS the crontab-removal branch exists only to retire this Mac's pre-#1764 lines. Once installed acceptance passes, it is a migration leftover; realign may delete it (and its test) then. Deleting it before acceptance would strand the old lines.
- Left alone as cosmetic: "cron" naming (`loopflow.cron.*` labels, `.lf/cron/`, `bootstrap-cron-host.sh`) on a platform that uses launchd. Renaming labels would need its own bootout migration for no user benefit.
- No open product choice.

## Demo decision pass 1 (2026-10-28, headless, standing in for Jack Heart): advance
- Observed: `uv run pytest python/tests/test_release_automation.py -k cron` — 8 passed, including the stopped-`crontab -` case (no agent loaded, Wave's cron lines kept). The branch diff moves the crontab removal ahead of `agent 9`/`agent 10`. PR #1768 open, checks green.
- Machine unchanged: both `# loopflow:infrastructure:` cron lines remain, no `loopflow.cron.*` agent loaded — consistent with the stopped headless bootstrap.
- Not proven here: installed acceptance needs a logged-in Terminal to approve macOS's crontab prompt (see Remains). That is Jack's action after merge, not a repairable code gap, so it does not justify another implement pass.
