# LOO-292 installed laptop demonstration — 2026-09-23

## Finish line

Use the published, installed `lf` to refresh the laptop, activate and inspect
the login/hourly job through `lf install schedule`, observe a real scheduled
or wake-recovery catch-up, and observe explicit installed catch-up after a
missed or failed automatic opportunity. Preserve caller edits, unpublished
commits, and this Task's existing worktree. Candidate binaries, simulated
plists, and fixture tests do not establish this proof.

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

## Disposition and remaining proof

Blocked on a published installation containing PR #1273, specifically commit
`c56340a142bcb6a854b98fed9b757cd9a6423f9a`. The latest release reported by the
supported status command is v0.12.19 and cannot supply the new installed path.
No release was started or retried; no binary was built or promoted and no job
was installed to invoke the obsolete internal transaction command.

Once a containing release is available, resume this same Task and worktree:
install through the supported published upgrade path, verify the installed
command surface, run the full refresh, and use `lf install schedule` to load
the login/hourly job. Record its real launchd execution and checkout/package
outcomes, then the explicit catch-up after an observed missed/failed
opportunity, including before/after preservation evidence. A loaded plist or
successful invocation alone is insufficient proof of catch-up.

The installed refresh, scheduled/wake catch-up, and explicit recovery remain
unproven. No implementation failure was exposed beyond the known release
dependency; no implementation edits were made. Existing Task files, branch
history, and canonical checkout bytes were preserved.
