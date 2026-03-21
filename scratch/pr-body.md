## Try it!

```bash
cargo test -p loopflow terminal_sessions -- --nocapture
swift test --package-path swift --filter GhosttyTerminalViewTests
swift test --package-path swift --filter LocalWaveServiceAuthTests
cd swift
xcodegen generate
xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO -only-testing:ConcertoTests/GhosttyTerminalViewTests -only-testing:ConcertoTests/RepoStateInteractiveSessionTests
```

What to look for:
- Rust attach tests verify tmux-backed sessions return `session_name`, `host`, `cwd`, and `status`.
- Swift tests verify Concerto builds `tmux attach-session -t <name>` locally and `ssh -t <host> "tmux attach-session -t <name>"` remotely.
- Xcode tests exercise the app target with the new terminal attach contract.

## Intent

Move terminal attach from a daemon-supplied launch command to a transport-agnostic connection contract. `lfd` should own terminal-session lifecycle and metadata, while Concerto decides how to attach to tmux locally or over SSH without sending terminal bytes through the daemon.

## Assumptions

- Tmux-backed sessions are the only attachable terminal sessions right now.
- The host a client uses for the HTTP API is also the host it should SSH to for remote tmux attach.
- Concerto/Ghostty remains the terminal participant; `lfd` only supervises lifecycle and emits events.

## Key decisions

- Added explicit terminal-session HTTP routes and DTOs instead of overloading existing session APIs.
- Normalized loopback hostnames to `localhost` so the client can consistently take the local tmux path.
- Return `412 Precondition Failed` for non-tmux attach attempts instead of inventing a fake launch payload.
- Kept the client-side attach command assembly in Concerto so remote/local transport policy stays in the client.

## Not included

- Harness server mode for non-terminal clients.
- SSH brokering or terminal byte forwarding through `lfd`.
- Broader executor regression coverage beyond the targeted terminal attach/session tests on this branch.
