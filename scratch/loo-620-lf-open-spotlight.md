# LOO-620: lf open, Spotlight and Finder reach only the installed Loopflow.app

Status: implemented and realigned (2026-11-02), awaiting gate. Source: Task definition
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
  `swift/dist/Loopflow.app` once the DMG exists. local-bin, DerivedData, rebuild and
  other raw copies are dev apps without remembering anything.
- **Promotion converges on one bundle.** Staged (`promote.*`) and superseded
  (`superseded.*`) copies live in `<Applications>/.Loopflow.app.noindex/`
  (same volume, so commit stays a rename). `install::converge_release_app(target)` runs at
  promotion start (clears an interrupted promotion's leftovers), after settlement,
  and after switch recovery: it removes that directory and the sibling
  `.Loopflow.app.promote.*` / `.superseded.*` copies earlier lf left beside the
  target; for targets in `/Applications` or `~/Applications` it also unregisters
  (`lsregister -u`) what it removes, removes `~/Applications/{Loopflow,Concerto}.app`
  carrying `com.loopflow.mac`/`com.loopflow.concerto`, and registers the target
  (`lsregister -f`). The legacy target stays with its existing owners
  (`settle_app_artifacts`, `recover_switch`).

## Realign 2026-11-02

- `installed_app()` reads the OS account's Home (like the rest of install), so on a
  Mac with Loopflow installed every `lf open` fixture reaches the real
  `/Applications/Loopflow.app` by path unless it sets `LF_DESKTOP`. The Desktop batch
  test in `work_open_tests` asserted the old `-b com.loopflow.mac` form and failed;
  it now addresses its own Desktop (`LF_DESKTOP=com.loopflow.mac.test`) like the
  `global_commands` delivery test, and its fake `osascript` probe is gone.
- The "two consecutive installs" acceptance only held on a Machine without earlier
  sibling orphans. `sibling_promotion_copies` now sweeps them, covered by the
  repeated-installs test.
- #1777 (`lf install --dev`) and #1656 (drag-and-open install) are both still open;
  nothing new on `origin/main` since the base. Rebase onto whichever lands first.

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

## Remaining

- Gate: affected suites (`global_commands`, `work_open_tests`, install lib tests,
  `test_release_loopflow.py`, `test_app_bundle.py`, `test_install_script.py`), clippy.
- Demo on a Mac: `lf install` twice, then `ls -a /Applications`, `lf open`.

Checks (2026-11-02 realign): `cargo test -p loopflow -- repeated_installs_leave_one_bundle available_conversations_need_no_question desktop_launches_the_installed desktop_inside_a_build` 4 passed; `cargo clippy -p loopflow --all-targets -- -D warnings` clean; Python suites unchanged since the implement pass.
