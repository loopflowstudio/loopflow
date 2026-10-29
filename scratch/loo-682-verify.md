# LOO-682 verify (2026-10-28)

- PR #1764 merged as 334c05cd0; ~/src/loopflow is clean on that commit.
- Installed acceptance is still unproven. The Machine runs both jobs from the old crontab lines, and no `loopflow.cron.infrastructure.*` LaunchAgent is loaded or installed.
- Bootstrap rerun (implement, 2026-10-28) passed installed lf 0.15.0, placement, tool path, provider authority and the publisher preflight ("Release host preflight passed"). Both LaunchAgents loaded. `crontab -` then blocked for over 40 minutes, apparently on a macOS permission prompt that nobody answered in the headless run. It was stopped (exit 143) with the crontab unchanged. Both agents were booted out and their plists moved to /tmp/loo-682/, so release-run is never scheduled twice. Telemetry never ran.
- Next: from a logged-in Terminal on this Mac, run `scripts/bootstrap-cron-host.sh infrastructure` in ~/src/loopflow and approve the crontab permission prompt. Then confirm the two loaded agents, that the Wave's crontab lines are gone, and a green kickstarted telemetry run.
- Finding: a headless bootstrap can hang indefinitely at `crontab -`. Removing the crontab lines before loading the agents, or failing fast when crontab is not writable, would turn that into a clear error.
- Finding: an lf-launched shell bakes its PATH (Loopflow Dev.app) into the regenerated wrappers. lf itself stays pinned to the install gate.
