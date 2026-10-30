# LOO-700 questions

- Executable name: chose `Loopflow Dev` (matches CFBundleName/display name) over `LoopflowDev`. A space is fine for macOS bundles and `pgrep -x "Loopflow Dev"`; reversible by changing `DEV_EXECUTABLE` in `scripts/loopflow-dev.py`.
- Check (after compress): `uv run pytest python/tests/test_loopflow_dev.py` — 9 passed. Installed-app proof (real build, relaunch, `pgrep -x Loopflow` misses Dev) left to demo; no Dev app relaunch here.
