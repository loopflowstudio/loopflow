# LOO-700 demo: Loopflow Dev's own process name (headless, 2026-11-02)

No one was available; the agent stood in for Jack Heart at demo. Per Jack's standing rule, no Dev app was launched or relaunched; the proof stops at a real installed bundle plus a real process named `Loopflow Dev`.

## Observed

- Real build: `loopflow-dev.py` `cmd_build` (FFI + `swift build`) succeeded.
- Real install: `_install_dev_app()` with `DEV_APP` redirected to a temp `Loopflow Dev.app` pre-seeded with a stale `Contents/MacOS/Loopflow` (the release binary). Result: `Contents/MacOS` = `lf`, `Loopflow Dev` only; stale `Loopflow` gone. Plist: `CFBundleExecutable=Loopflow Dev`, `CFBundleIdentifier=com.loopflow.mac.dev`, `CFBundleName=Loopflow Dev`. `codesign --verify --deep --strict` passed; signature `Identifier=com.loopflow.mac.dev`, same Dev signing identity path. Sparkle embedding ran (LOO-699 merged into base).
- Release app untouched: `/Applications/Loopflow.app` still `CFBundleExecutable=Loopflow`.
- Process name: a compiled stand-in executable named `Loopflow Dev` running beside the release app (pid 99798): `pgrep -x Loopflow` → only 99798; `pgrep -x "Loopflow Dev"` → only the stand-in.
- `uv run pytest python/tests/test_loopflow_dev.py` — 9 passed.

## Not proved here

Quit/relaunch of the installed `~/Applications/Loopflow Dev.app` (it still holds the old `Loopflow` binary). Jack's next `loopflow-dev.py` run upgrades it; `_stop_loopflow_app` matches both old and new names by the `Contents/MacOS/Loopflow` prefix.

## Decision

Advance. Acceptance criteria hold on the real build/install path; the remaining relaunch is Jack's next ordinary Dev run, not a product question.
