# LOO-620: lf open, Spotlight and Finder reach only the installed Loopflow.app

Status: implemented, compressed, synced onto main through #1792 and realigned (2026-11-02); gate next. Source: Task definition
(Jack Heart, 2026-10-26; extended 2026-11-02 after the Desktop database-mismatch incident).

## What exists

- **`lf open` / `lf desktop` launch by path.** `open::DesktopApp` is the one Desktop
  destination: `Running(bundle)` when `LF_DESKTOP` names the caller's own Desktop
  (`open -b`), else `Installed(path)` from `installation::installed_app()`, the
  bundle the OS account's `~/.local/bin/lf` resolves into (`open -a`). No installed
  app and no `LF_DESKTOP` is a definite failure, so the route moves on. The link
  receiver asks the same question. Socket path keeps `desktop_bundle()`.
- **Only release packaging carries `com.loopflow.mac`.** `Info.plist` and
  `project.yml` say `com.loopflow.mac.dev`; `scripts/release-loopflow.py` stamps the
  release id (`bundle_version.stamp_release_bundle_id`) and deletes
  `swift/dist/Loopflow.app` once the DMG exists. `scripts/install.py local` stages
  its app under `local-bin/app.noindex/`, which Spotlight skips, and removes the
  `local-bin/Loopflow.app` earlier builds left beside `lf`. DerivedData, rebuild and
  other raw copies are dev apps without remembering anything.
- **Promotion converges on one bundle.** Staged (`promote.*`) and superseded
  (`superseded.*`) copies live in `<Applications>/.Loopflow.app.noindex/`
  (same volume, so commit stays a rename). `install::converge_release_app(target)` runs
  once per promotion, after settlement, and again after switch recovery: it removes
  that directory (including any copy an interrupted promotion left) and the sibling
  `.Loopflow.app.promote.*` / `.superseded.*` copies earlier lf left beside the
  target; for targets in `/Applications` or `~/Applications` it also unregisters
  (`lsregister -u`) what it removes, removes `~/Applications/{Loopflow,Concerto}.app`
  carrying `com.loopflow.mac`/`com.loopflow.concerto`, and registers the target
  (`lsregister -f`). The legacy target stays with its existing owners
  (`settle_app_artifacts`, `recover_switch`).

## Tests

- `installed_app()` reads the OS account's Home (like the rest of install), so on a
  Mac with Loopflow installed every `lf open` fixture reaches the real
  `/Applications/Loopflow.app` by path unless it sets `LF_DESKTOP`. The Desktop
  fixtures in `work_open_tests` and `global_commands` address their own Desktop
  (`LF_DESKTOP=com.loopflow.mac.test`); the fake `osascript` probe is gone.
- The "two consecutive installs" acceptance only holds on a Machine with earlier
  sibling orphans swept, so the repeated-installs test plants one.

## Open branches nearby

#1777 (`lf install --dev`) and #1656 (drag-and-open install) are both still open.
Synced onto main through #1792 (Loopflow Dev under its own process name): it
stamps `com.loopflow.mac.dev`, the id `project.yml` now defaults to, so Loopflow
Dev stays a dev app with no branch change. Rebase onto whichever lands first.

## Deleted

- `link_receiver::application_for_bundle` (bundle-id lookup of "installed").
- `DesktopBatch::installed` bundle-id probe, replaced by `DesktopApp`.
- Sibling `.Loopflow.app.promote.*` / `.superseded.*` naming in `/Applications`, replaced
  by the `.noindex` scratch directory.

## Acceptance → evidence

- Stale same-id copy registered → `lf open` launches `/Applications/Loopflow.app`:
  `open::tests::desktop_launches_the_installed_app_by_path` (args are `-a <path>`).
- Xcode build non-production id, packaged app `com.loopflow.mac`:
  `python/tests/test_release_loopflow.py::test_only_release_packaging_carries_the_installed_bundle_id`.
- Two consecutive installs leave only `Loopflow.app`, even beside an earlier orphan:
  `install::artifact_tests::repeated_installs_leave_one_bundle_in_the_applications_folder`.

## Contract

`lf open` has one destination, the installed app by path, and a bundle id names
only the Desktop the caller runs inside. Promotion converges on one bundle. The
error for a Mac without Desktop (`lf install` places it) is the only new copy.
Checked on Jack's Mac 2026-11-02 (read-only): `~/.local/bin/lf` resolves into
`/Applications/Loopflow.app`; `mdfind` on `com.loopflow.mac*` lists only
`/Applications/Loopflow.app` and `~/Applications/Loopflow Dev.app`; no
`.promote.*`/`.superseded.*` orphans, so the sweep's proof stays the unit test.

Not changed, and why:

- `DesktopApp::Running` is reached by id even for the installed Desktop (its agents get
  `LF_DESKTOP=com.loopflow.mac`); Launch Services prefers the running instance for
  `-b`, which is the one that launched the agent. Accepted in the Task text.
- `quiesce_app_processes` asks `application id "com.loopflow.mac"` to quit, then
  signals by executable path; stays as `scratch/questions.md` records.

## Remaining

- Gate: affected suites (`global_commands`, `work_open_tests`, install lib tests,
  `test_release_loopflow.py`, `test_app_bundle.py`, `test_install_script.py`), clippy.
  `global_commands::scheduled_install_is_independent_of_the_invoking_checkout_and_reusable`
  fails on main too (#1776's HOME-vs-account-home check, macOS-only); not this branch.
- Demo on a Mac: `lf install` twice, then `ls -a /Applications`, `lf open`.

Checks (2026-11-02 compress): `cargo test -p loopflow --lib -- install::artifact_tests:: open::tests:: installation::` 36 passed; `cargo test --test integration -- available_conversations_need_no_question` and `--test global_commands -- open_delivers_exact_task_to_desktop` passed; clippy clean; Python suites unchanged since the last pass (35 passed), gate reruns them.
