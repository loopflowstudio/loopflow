# Wave crons review — Validation

```bash
cargo fmt --check
cargo clippy -- -D warnings
cargo test --all
uv run pytest python/tests/
uv run pytest python/tests/test_client.py -q
uv run pytest tests/e2e/test_api_smoke.py tests/e2e/test_concurrent_clients.py -v
swift test --package-path swift
cd swift && xcodegen generate && xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO -only-testing:ConcertoTests
```

Known flake: full `xcodebuild test` (without `-only-testing`) fails locally with `ConcertoUITests-Runner` early exit before bootstrapping.
