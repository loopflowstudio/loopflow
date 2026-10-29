# LOO-682: scheduled jobs reach the login Keychain

Status: implemented, unpublished.

## Decision (2026-10-28)

On macOS `scripts/bootstrap-cron-host.sh` installs each job as a gui-domain
LaunchAgent (`loopflow.cron.<wave>.<job>`, `StartCalendarInterval` 09:00/10:00,
`ProcessType Standard`, `AbandonProcessGroup` so publisher children outlive
the parent like under cron), then removes the Wave's `# loopflow:<wave>:` crontab
lines. Linux keeps crontab. Both run the same generated `.lf/cron/` wrapper
(installed gate, `env -i` minimal env, LF_HOME). The bootstrap's telemetry check
now `launchctl kickstart`s the loaded agent and waits for its exit code, so it
runs in the scheduled context. Reusing the `loopflow.cron.*` labels replaces the
old launchd-retirement loop.

## Evidence

- Cause: both 2026-10-28 failures are Keychain reads/writes from cron
  (`.lf/logs/release-run.log`, `.lf/logs/telemetry-daily.log`).
- One-shot gui-domain launchd probes (no calendar, booted out after): login
  Keychain info, the publisher's keyring token and the Claude item all read;
  `loopflow-release-publisher uv run python scripts/publish_release.py check`
  in the wrapper's minimal env exited 0 ("Release host preflight passed").
- Not done: no cron-context probe (would need an extra crontab schedule).

## Remaining

- After merge, from `~/src/loopflow` on main: `scripts/bootstrap-cron-host.sh
  infrastructure`. Expect two `loaded loopflow.cron.infrastructure.*` lines,
  the crontab lines removed (unrelated entries kept) and a green kickstarted
  telemetry run. LOO-645 confirms two real scheduled releases.
- `scripts/disk-cleanup.sh` still installs a crontab entry; out of scope unless
  it needs Keychain.

Check: `uv run pytest python/tests/test_release_automation.py python/tests/test_auth_cron_evidence.py -q` → 22 passed.
