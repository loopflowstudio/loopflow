# Gate validation — jack-heart.clear-the-deck.20260317_1502

## Validation run

- `git diff --check` ✅
- `cargo fmt --all` ✅
- `cargo clippy --all-targets -- -D warnings` ✅
- `cargo test --all` ✅
- `cargo test -p loopflow docker_` ✅
- `uv run pytest python/tests/` ✅
- `tests/e2e/test_smoke.sh` ✅
- `uv run pytest tests/e2e/test_api_smoke.py tests/e2e/test_concurrent_clients.py -v` ✅
- `swift test --package-path swift` ✅
- `cd swift && xcodegen generate && xcodebuild test -derivedDataPath /tmp/loopflow-xcode-dd-20260317-2 -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO`
  - XCTest suites passed, then `ConcertoUITests-Runner` exited during bootstrap before establishing a UI-test connection

## Measure

```bash
wc -l docs/lfd.md deploy/README.md docker/docker-compose.yml
```

Observed on this branch: 471 total lines across those three files.
