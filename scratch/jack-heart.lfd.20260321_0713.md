# Tmux attach helper polish

## Validate

```bash
cargo fmt --check
cargo clippy -p loopflow -- -D warnings
cargo test -p loopflow terminal_sessions -- --nocapture
swift test --package-path swift --filter GhosttyTerminalViewTests
swift test --package-path swift --filter LocalWaveServiceAuthTests
cd swift
xcodegen generate
xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO -only-testing:ConcertoTests/GhosttyTerminalViewTests -only-testing:ConcertoTests/RepoStateInteractiveSessionTests
```
