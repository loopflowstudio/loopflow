# Tmux attach helper polish

## Goal

Tighten the tmux attach follow-up without reopening the larger transport change.
Keep the remaining diff small, realistic, and easy to review.

## Done when

- Rust terminal-session route tests only synthesize `tmux_name` for tmux-backed fixtures
- Concerto models tmux attach launches as an explicit `{workingDirectory, argv, env}` bundle
- Local and remote tmux attach tests assert the env stays empty
- Terminal workspace keeps the existing Loopflow dark terminal chrome

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

## Notes

This branch is a polish pass on top of the shipped tmux attach contract, not a new daemon/client protocol change.
