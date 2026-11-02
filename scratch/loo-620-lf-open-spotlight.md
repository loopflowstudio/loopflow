# LOO-620: lf open, Spotlight and Finder reach only the installed Loopflow.app

Status: implemented (2026-11-02), awaiting gate. Source: Task definition
(Jack Heart, 2026-10-26; extended 2026-11-02 after the Desktop database-mismatch incident).

## What changed

- **`lf open` / `lf desktop` launch by path.** `open::DesktopApp` is the one Desktop
  destination: `Running(bundle)` when `LF_DESKTOP` names the caller's own Desktop
  (`open -b`), else `Installed(path)` from `installation::installed_app()` — the
  bundle `~/.local/bin/lf` resolves into (`open -a`). No installed app and no
  `LF_DESKTOP` is a definite failure, so the route moves on. The link receiver asks
  the same question. Socket path keeps `desktop_bundle()`.
- **Only release packaging carries `com.loopflow.mac`.** `Info.plist` and
  `project.yml` say `com.loopflow.mac.dev`; `scripts/release-loopflow.py` stamps the
  release id (`bundle_version.stamp_release_bundle_id`) and deletes
  `swift/dist/Loopflow.app` once the DMG exists. local-bin, DerivedData, rebuild and
  other raw copies are dev apps without remembering anything.
- **Promotion converges on one bundle.** Staged (`promote.*`) and superseded
  (`superseded.*`) copies live in `<Applications>/.Loopflow.app.noindex/`
  (same volume, so commit stays a rename). `install::converge_release_app(target)` runs at
  promotion start (clears an interrupted promotion's leftovers), after settlement,
  and after switch recovery: it removes that directory and, for targets in
  `/Applications` or `~/Applications`, unregisters and removes
  `~/Applications/{Loopflow,Concerto}.app` carrying `com.loopflow.mac`/`com.loopflow.concerto`
  (`lsregister -u`), then registers the target (`lsregister -f`). The legacy target
  stays with its existing owners (`settle_app_artifacts`, `recover_switch`).

## Delete — do not maintain

- `link_receiver::application_for_bundle` (bundle-id lookup of "installed") — deleted.
- `DesktopBatch::installed` bundle-id probe — replaced by `DesktopApp`.
- Sibling `.Loopflow.app.promote.*` / `.superseded.*` naming in `/Applications` — replaced
  by the `.noindex` scratch directory.

## Acceptance → evidence

- Stale same-id copy registered → `lf open` launches `/Applications/Loopflow.app`:
  `open::tests::desktop_launches_the_installed_app_by_path` (args are `-a <path>`).
- Xcode build non-production id, packaged app `com.loopflow.mac`:
  `python/tests/test_release_loopflow.py::test_only_release_packaging_carries_the_installed_bundle_id`.
- Two consecutive installs leave only `Loopflow.app`:
  `install::artifact_tests::repeated_installs_leave_one_bundle_in_the_applications_folder`.

## Remaining

- Gate: affected suites, clippy.
- Demo on a Mac: `lf install` twice, then `ls -a /Applications`, `lf open`.

Checks: focused Rust lib tests (open::tests, install repeated_installs), `global_commands open_delivers`, `test_release_loopflow.py`, `test_app_bundle.py`, `test_install_script.py` — pass.
