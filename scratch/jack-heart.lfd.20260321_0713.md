# LFD: Session Lifecycle + Client Access — Validation

## Done when (Milestone 1)

- `TerminalLaunchSpecDto` is deleted
- Attach endpoint returns `TerminalConnectionInfo`
- Concerto connects to tmux sessions using connection info (local attach works)
- No terminal bytes flow through `lfd`

## Try it

```bash
# Verify TerminalLaunchSpecDto is gone
rg "TerminalLaunchSpecDto|TerminalLaunchSpec" rust swift docs scratch

# Rust attach tests
cargo test -p loopflow terminal_sessions -- --nocapture

# Swift package tests
swift test --package-path swift --filter GhosttyTerminalViewTests
swift test --package-path swift --filter LocalWaveServiceAuthTests

# Concerto app tests
cd swift
xcodegen generate
xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO -only-testing:ConcertoTests/GhosttyTerminalViewTests -only-testing:ConcertoTests/RepoStateInteractiveSessionTests
```

What to look for:
- Rust attach tests verify tmux-backed sessions return `session_name`, `host`, `cwd`, and `status`.
- Swift tests verify Concerto builds `tmux attach-session -t <name>` locally and `ssh -t <host> "tmux attach-session -t <name>"` remotely.
- Xcode tests exercise the app target with the new terminal attach contract.
