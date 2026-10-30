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

## Remaining

- New PR on this Task (#1794 is merged).
