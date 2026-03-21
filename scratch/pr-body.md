## Try it!

```bash
# Core validation
cargo fmt --check
cargo clippy -- -D warnings
cargo test --all
cargo test -p loopflow docker_
uv run pytest python/tests/
tests/e2e/test_smoke.sh
uv run pytest tests/e2e/test_api_smoke.py tests/e2e/test_concurrent_clients.py -v
swift test --package-path swift

# Concerto app + Xcode coverage
uv run python scripts/concerto-dev.py run-debug
cd swift && xcodegen generate && xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO
```

In Concerto:
- open a repo window with no selected wave and confirm the empty queue shows wave overview cards instead of the old detail-panel empty state
- select a wave and confirm the default workspace opens with **Roadmap**, **Runs**, and **Terminal** panes
- press **Cmd+K** and switch waves or focus/create panes like **README**, **Launcher**, and extra **Terminal** panes
- drive a wave into an interactive step and confirm the queue item shows step-specific detail, the workspace auto-takes over with the live session, and the item resolves when the session finishes
