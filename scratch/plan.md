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
