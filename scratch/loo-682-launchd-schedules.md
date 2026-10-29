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

## Concept review (2026-10-28)

The fix is right for the release host, but the lesson is product-wide: any
scheduled lf command that launches an agent or a Doppler-backed publisher on
macOS needs the login Keychain, and cron never has it. Current guidance
recommends crontab anyway:

- `docs/work.md` ("For a schedule, put an ordinary command in your own
  crontab", example `lf -b --wave billing wave-operate`, which launches Claude).
- `rust/loopflow/src/builtins/ops/skill/init.md` ("Scheduled commands are
  ordinary crontab lines on the Machine that runs them").
- `docs/ops.md` names launchd but shows only crontab.

Done: `docs/work.md`, `docs/ops.md` and the builtin `init` skill keep "Loopflow
keeps no scheduler" and say that on macOS a command that runs an agent or reads
Keychain credentials belongs in a gui-domain LaunchAgent
(`StartCalendarInterval`); crontab stays the Linux/no-Keychain answer. ops.md
keeps `lf pr reconcile` (needs only `gh`) in crontab and shows the release as a
LaunchAgent plist.

Considered and left: `lf install`'s Rust LaunchAgent (`com.loopflow.refresh`,
`install/published.rs`) is a second plist writer, but it is product surface for
a different job, while the bootstrap is a repository maintenance script;
sharing would make the script depend on a hidden lf command. The
`loopflow.cron.*` labels and `.lf/cron/` wrappers keep "cron" as the Wave's
scheduled-job word (GOAL.md `## Cron`); renaming them would orphan the loaded
agents for no user gain.

Second pass (graph step, 2026-10-28): implemented docs, skill and bootstrap
match this review; no further change and no open product choice.

## Considered and left (scope)

- `scripts/disk-cleanup.sh` still installs a crontab entry; it runs no agent and its `lf wt prune` steps already run from cron, so it stays out of scope.

## Remaining

- After merge, from `~/src/loopflow` on main: `scripts/bootstrap-cron-host.sh
  infrastructure`. Expect two `loaded loopflow.cron.infrastructure.*` lines,
  the crontab lines removed (unrelated entries kept) and a green kickstarted
  telemetry run. LOO-645 confirms two real scheduled releases.

Check: `uv run pytest python/tests/test_release_automation.py python/tests/test_auth_cron_evidence.py -q` → 22 passed. `cargo test -p loopflow --test integration documented_commands::` → 4 passed; `--lib skills::catalog::` → 8 passed.
