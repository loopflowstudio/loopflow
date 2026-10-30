# LOO-699 demo: every Loopflow.app build launches (2026-10-30)

Headless demo, decision pass 1. No one was available; the agent stood in for
Jack Heart. PR #1794 is merged (`646b20fb24`). Its acceptance covers three builders.

## Observed through the real builders

| Builder | Run | Bundle | Launch (`-ui-test-mode empty-workspaces`, temp `LF_HOME`, check bundle id) |
| --- | --- | --- | --- |
| `scripts/install.py local` | real command, 126 s | Sparkle.framework with version symlinks, rpaths `@loader_path` + `@executable_path/../Frameworks`, Developer ID, `codesign --verify --deep --strict` ok | exit 0, 73 KB snapshot |
| `loopflow-dev.py` `cmd_build` + `_install_dev_app` | real functions, `DEV_APP` pointed at a temp dir; `~/Applications/Loopflow Dev.app` untouched (mtime unchanged) | same framework and rpaths, strict verify ok | exit 0, snapshot |
| `release-loopflow.py` `release()` | real function, stopped after its own resource/launch check, before DMG and notarization | Developer ID, hardened runtime on app and framework, same TeamIdentifier, strict verify ok | the script's own check passed ("Verified packaged app renders") |
| bare `swift/.build/release/LoopflowMac` in a bundle (before-fix control) | copied as the old builders did | no Frameworks | dyld abort: `Library not loaded: @rpath/Sparkle.framework/Versions/B/Sparkle`; `verify_rpath_libraries` raises `cannot load Sparkle.framework/Versions/B/Sparkle` |

The demo shows the accepted three-builder behavior works.

## Gap: two more builders still ship the bare binary

The Task's title says *every* way of building Loopflow.app. Two more builders
copy `swift/.build/release/LoopflowMac` into `Contents/MacOS` and never embed Sparkle,
so their app aborts in dyld exactly like the control above:

- `scripts/benchmarks/desktop-performance/launch.py` `bundle()` (the documented
  `launch.py run` launch benchmark; `python/tests/test_desktop_launch.py`).
- `scripts/rebuild_third_party.py` `build()`. `scripts/third_party.py` ships it to
  recipients as `rebuild.py` for third-party relinking. This one is shipped, and the
  relinked app it produces cannot start.

Interpretation (agent, not Jack): this is a concrete, repairable gap in the Task's
stated outcome, so the decision is **iterate**.

## Implementation pass 2 (2026-10-30)

Branch restarted from origin/main `646b20fb24`; the merged branch's history held nothing main lacks.

- `launch.py bundle()` and `rebuild_third_party.py build()` call `app_bundle.embed_frameworks`
  before signing. `third_party.py` ships `app_bundle.py` beside `rebuild.py` and `verify()`
  requires it; REBUILD.md says so; notices inventory refreshed for REBUILD.md.
- Every `.build/release/LoopflowMac` copy in `scripts/` now embeds (install, dev, release,
  launch benchmark, rebuild). Xcode builds (`generate_screenshots.py`,
  `check_attributegraph_cycles.sh`) embed through Xcode; `prove_wave_surface_states.sh` runs
  the SwiftPM binary in place, where `@loader_path` resolves.
- `test_app_bundle.py` imports both scripts plainly and runs both builders on a real SwiftPM-shaped Mach-O and launches the
  result with the build directory deleted; both tests fail with the embed call removed.

Check: `uv run pytest python/tests/test_app_bundle.py python/tests/test_third_party.py python/tests/test_desktop_launch.py` → 21 passed (again after compress, 2026-10-30).
Not run: a real `swift build` through either builder (release binary already proven in pass 1); left to demo.

## Concept review (2026-10-30, headless)

Users meet one thing: *a Loopflow.app built from this source starts*. The code
holds that as two separable steps in five builders: copy `LoopflowMac` under the
`CFBundleExecutable` name, then `embed_frameworks`. The incident was a builder
doing the first without the second; pass 2 fixed it by adding the second call twice
more. The next builder (or the next linked framework) repeats the incident, because
nothing joins the steps. Naming the bundle executable has five spellings too:
`release-loopflow.py _bundle_executable` (plistlib), `loopflow-dev.py
_bundle_executable_name` (PlistBuddy), `rebuild_third_party.py` inline plistlib,
`install.py APP_NAME` + `_bundle_binary_name`, and `launch.py`'s literal `Loopflow`.

Settled by the accepted outcome and AGENTS.md's one-implementation rule (agent,
not Jack): **the app executable and its frameworks are one placement.**

Next implement direction:

- `app_bundle.install_app_executable(build_dir, info_plist, contents) -> Path`:
  reads `CFBundleExecutable`, fails naming the missing `LoopflowMac`, copies it to
  `Contents/MacOS/<name>`, embeds and verifies, returns the executable path.
  `embed_frameworks` becomes private to `app_bundle.py`.
- All five builders call it; delete `_bundle_executable`, `_copy_app_executable`,
  `_bundle_executable_name`, launch.py's literal, and install.py's app-binary branch
  of `_bundle_binary_name` (`lf`/auxiliary tools keep `_atomic_install`; the app dir
  is removed first, so the app binary needs no atomic swap).
- Dev's stale-`LoopflowMac` cleanup stays in `loopflow-dev.py` (dev reuses its bundle).
- Stays per builder: Info.plist stamping/bundle-id overrides, icon, resource bundles,
  `strip`, signing. They differ for real reasons; folding them in would make a
  builder framework nobody asked for.
- Existing tests keep their real-Mach-O launch proof; one `app_bundle` test covers
  the missing-product and name-from-plist cases.

Counterexample checked: the shipped third-party `rebuild.py` already imports
`app_bundle.py` beside it, so the shared call costs recipients nothing new.

No open product choice.

## Implementation pass 3: one placement (2026-10-30)

`app_bundle.install_app_executable(build_dir, info_plist, contents)` is the only way
the app executable enters a bundle; `_embed_frameworks` is private and
`app_bundle.executable_name` is the one CFBundleExecutable reader (release's check copy,
install's layout check, launch.py's `launch()`). All five builders call it. Deleted:
release `_bundle_executable`/`_copy_app_executable` (and its unit test), dev
`_bundle_executable_name`, install `_bundle_binary_name`/`SWIFT_APP_PRODUCT`
(`BundleSpec.executables` became `tools`), launch.py's `MacOS/Loopflow` literals.

Check: `uv run pytest python/tests/test_app_bundle.py test_third_party.py test_desktop_launch.py test_install_script.py test_release_loopflow.py test_loopflow_dev.py` → 49 passed; `grep embed_frameworks` hits only `scripts/app_bundle.py`.

## Remaining

- Publish as a new PR on this Task (#1794 is merged).
