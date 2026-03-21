# Terminal attach connection contract — validation

## Validation

- `rg "TerminalLaunchSpecDto|TerminalLaunchSpec" rust swift docs wave` ✅
- `cargo fmt --check` ✅
- `cargo clippy -- -D warnings` ✅
- `cargo test -p loopflow terminal_sessions -- --nocapture` ✅
- `cargo test --all` ✅
- `swift test --package-path swift --filter GhosttyTerminalViewTests` ✅
- `swift test --package-path swift --filter LocalWaveServiceAuthTests` ✅
- `swift test --package-path swift` ✅
- `cd swift && xcodegen generate` ✅
- `cd swift && xcodebuild test ... -only-testing:ConcertoTests/GhosttyTerminalViewTests -only-testing:ConcertoTests/RepoStateInteractiveSessionTests` ❌ bootstrap crash before tests start
