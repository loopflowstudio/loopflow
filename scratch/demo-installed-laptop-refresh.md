# LOO-292 installed laptop demonstration — 2026-09-23

Historical evidence; superseded requirements and blockers below are retained as
dated observations. The [current plan](make-laptop-refresh-and-lf-installed-schedule-proof.md)
and [latest readback](demo-published-install-20261002.md) govern remaining proof.


## Live observations

Checked at approximately 2026-09-23T20:10:56Z on the laptop (uid 501).

- Task branch: `jack-heart/make-laptop-refresh-and-lf-installed-schedule-proof`.
  Initial HEAD: `1694fc7409da704548d26d75797e9ff2ce9c419b`.
  Initial `git status --short` was empty; the prior demonstration note remains
  intact at `scratch/demo-laptop-refresh.md`.
- `command -v lf`: `/Users/jack/.local/bin/lf`, a symlink to
  `/Users/jack/.lf-machine/install/gates/1/lf`.
- Installed `lf --version`: `lf 0.12.19`.
- Installed `lf install --help`: `Internal installer transaction entry point`;
  no schedule subcommand. `lf install schedule --help` exits 2 with
  `error: unrecognized subcommand 'schedule'`.
- `lf release status` succeeds and reports target `default`, latest tag
  `v0.12.19`, workflow `completed / success`, release notes
  `narrative / gate safe`, and `GitHub Release: yes`.
  Hosted workflow: https://github.com/loopflowstudio/loopflow/actions/runs/35894647999.
  This command refreshes release tags from origin; it does not start a release.
- A live HTTP request to the installer's `releases/latest` URL redirects to
  https://github.com/loopflowstudio/loopflow/releases/tag/v0.12.19,
  independently confirming the published upgrade destination.
- Release tag `v0.12.19` resolves to
  `bca3f35ac7d4ad24ebed2a30481d0b7ba3a86bde`
  (`2026-09-23T17:09:48Z`). PR #1273 merged as
  `c56340a142bcb6a854b98fed9b757cd9a6423f9a`
  (`2026-09-23T19:50:01Z`). Their merge base is the release commit:
  the published tag predates and excludes the change.
- Canonical checkout `/Users/jack/src/loopflow` has clean status and HEAD
  `0bd9bb90d46a0638c54c042c6b26e94e0b25e8d7`; it also predates #1273.
  No checkout refresh was attempted with the old executable.
- `~/Library/LaunchAgents/com.loopflow.refresh.plist` is absent.
  `launchctl print gui/501/com.loopflow.refresh` exits 113 and reports
  `Could not find service "com.loopflow.refresh" in domain for user gui: 501`.
  This proves no configured job at inspection time, not a failed scheduled run.

## Historical disposition

The September 23 review stopped at the v0.12.19 release dependency without
installation, activation or implementation changes. Task history and checkout
bytes were preserved. Superseded hourly/full-refresh instructions remain in
`8111e7f31ff235dfe35d56406a62bd585ab74e20:scratch/demo-installed-laptop-refresh.md`.
The current plan owns remaining proof; the October 2 readback supersedes this blocker.
