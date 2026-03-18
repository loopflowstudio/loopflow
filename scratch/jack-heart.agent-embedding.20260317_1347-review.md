# Validation: jack-heart.agent-embedding.20260317_1347

## Automated

| Suite | Result |
|-------|--------|
| `cargo fmt --check` | pass |
| `cargo clippy -- -D warnings` | pass |
| `cargo test --all` | pass |
| `uv run pytest python/tests/` | 113 passed |
| `swift test --package-path swift` | 243 passed |
| `tests/e2e/test_smoke.sh && uv run pytest tests/e2e/test_api_smoke.py tests/e2e/test_concurrent_clients.py -v` | smoke pass + 16 passed |
| `cd swift && xcodegen generate && xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO` | default DerivedData run failed locally with a stale-output write error |
| `cd swift && xcodegen generate && xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' -derivedDataPath /tmp/LoopflowSwiftGate.<ts> CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO` | app build + unit suites completed, then `ConcertoUITests-Runner` hung before establishing connection in this no-rendering environment |

## Manual product check

Run:

```bash
uv run python scripts/concerto-dev.py run-debug
```

Verify:

1. Selecting a wave opens the work surface instead of a terminal takeover.
2. The Terminal tab appears only when the selected wave has a terminal session.
3. Exiting the terminal with status 0 resumes the wave; non-zero marks it failed.
4. No selection shows the repo-wide attention queue.
5. Attention items render with the backend kind they came from.
