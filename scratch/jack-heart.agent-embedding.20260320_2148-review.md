# Review: Validation

Passed locally:

- `cargo fmt --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `.venv/bin/pytest python/tests/` (115 passed)
- `cargo test -p loopflow ops::ingest::tests` (12 passed)
- `swift test --package-path swift` (311 passed)
- `swift test --package-path swift --filter WaveContentParser` (6 passed)
- `swift test --package-path swift --filter Multiplexer` (22 passed)

Environment-limited (not completed in sandbox):

- `cargo test --all` — sandbox blocks listener / Unix socket binding
- `xcodebuild test` — sandbox blocks cache writes and package fetching
