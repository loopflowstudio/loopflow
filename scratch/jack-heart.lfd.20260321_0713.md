# LFD: Terminal Attach Connection Contract — Validation

## Done when

- `TerminalLaunchSpecDto` is deleted
- Attach endpoint returns `TerminalConnectionInfo`
- Concerto connects to tmux sessions using connection info (local attach works)
- No terminal bytes flow through `lfd`

## Try it

```bash
rg "TerminalLaunchSpecDto|TerminalLaunchSpec" rust swift docs wave
cargo test -p loopflow terminal_sessions -- --nocapture
swift test --package-path swift --filter GhosttyTerminalViewTests
swift test --package-path swift --filter LocalWaveServiceAuthTests
cd swift
xcodegen generate
xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO -only-testing:ConcertoTests/GhosttyTerminalViewTests -only-testing:ConcertoTests/RepoStateInteractiveSessionTests
```

- Rust attach tests verify tmux-backed sessions return `session_name`, `host`, `cwd`, and `status`.
- Swift tests verify Concerto builds `tmux attach-session -t <name>` locally and `ssh -t <host> "tmux attach-session -t <name>"` remotely.
- Xcode tests exercise the app target with the new terminal attach contract.
