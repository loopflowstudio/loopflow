# LOO-800 launch-and-quit PR check

Built:
- `scripts/check_desktop_launch.py` owns `check_launch(executable, work_dir)`:
  launch, exit 0, snapshot written, else raise (hang: 30 s, `sample`, killpg).
  `release-loopflow.py` calls it with build resources hidden; its own launcher,
  env and exit/snapshot checks are deleted.
- `macos.yml` `swift-test`: "Launch Desktop and wait for it to quit" after the
  headless tests, against `swift/.build/debug/LoopflowMac`.
- Docs: `.github/CI.md` row, `swift/README.md` Testing paragraph.

Checks:
- `uv run pytest python/tests/test_check_desktop_launch.py python/tests/test_release_loopflow.py` — 18 passed (after compress too).
- `uv run python scripts/check_desktop_launch.py`, local `swift build -Xswiftc -gnone` debug app, current tree: pass, 5 s.
- Same, with #1846's `swift/` changes reverted: fails at 35 s; stack shows `View.uiTestSnapshot()` → `-[NSApplication terminate:]` → `nextEventMatchingMask:`. Debug build suffices; no release build needed.
- Hosted-runner launch and added time: first PR run (gate/CI); state the time in the PR.

Gate plan (2026-11-08):
- Launcher/release scripts and their tests → focused launch/release pytest files;
  macOS classifier → `test_macos_inputs.py`; workflow → `test_release_automation.py`.
- `.github/CI.md`, Swift Testing docs, QA skill and memory → architecture check.
  No Rust or Swift production source changed; no Rust tests/clippy or full Swift
  model suite needed. Build Desktop and run the real launch check once.
- `scripts/test.py --list` selects all Python tests and the full Swift suite
  (the latter from README alone); use the focused commands above because this
  runner has no narrowing options for Python/Swift. Ruff checks changed Python.
- Keep the recorded 34–35 s reverted-fix regression; gate does not repeat it.
- PR #1850 at `e89a41b93`: CI run 42446708802 deferred the matrix, including
  macOS, because committed scratch is present. Hosted success and added time
  remain delivery/CI work after scratch cleanup, not a green gate result.
- First gate `swift build --package-path swift -Xswiftc -gnone` failed linking:
  `target/debug/libloopflow_ffi` was absent. Build the library first, as CI does;
  the QA skill now includes this prerequisite.

Gate results (all commands with inherited `LF_*` unset and stdin closed):
- `uv run --no-sync python scripts/test_network.py uv run --no-sync pytest python/tests/test_check_desktop_launch.py python/tests/test_release_loopflow.py python/tests/test_macos_inputs.py python/tests/test_release_automation.py` — 57 passed, 22.11 s.
- `cargo build -p loopflow-ffi --lib --jobs 4` — passed, 90 s; required missing app-link prerequisite.
- `swift build --package-path swift -Xswiftc -gnone` — passed after the prerequisite, 3.53 s; existing Swift warnings and native objects targeting 26.2 versus Swift's 26.0 remain outside this diff.
- `/usr/bin/time -p uv run python scripts/check_desktop_launch.py` — passed, 10.00 s; snapshot written and app exited 0.
- `uv run python scripts/check_architecture.py` — passed, all seven owner/boundary categories covered.
- `uv run ruff check` and `uv run ruff format --check` on the six changed Python files — passed; `git diff --check` passed.

Gate repaired stale captures passing on a reused directory and added the launch
script to macOS job selection. Cached PR copy includes these repairs and the
hosted CI deferral. No publication or landing performed.
