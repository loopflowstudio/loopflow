# Open questions

- `uv run pytest tests/e2e/test_fork.py -v` did not complete in this environment (process exited with signal 143 while running the long Docker-backed scenario). Re-run in a dedicated Docker + Claude-credential environment to validate the new pytest wrapper end-to-end.
- Can `ConcertoUITests/ScreenshotPipelineTests.testCapture` be stabilized in this environment so full `xcodebuild test -scheme Concerto` passes reliably?
- Should reconnect promotion from `.reconnecting` to `.live` stay timer-based (1s fallback) or move to an explicit replay/live boundary signal?
