## Try it!

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

What to look for:
- Rust attach tests now model non-tmux sessions without a synthetic `tmux_name` and still reject attach with `412 Precondition Failed`.
- Swift tmux attach tests verify both local and remote commands carry an empty env dictionary.
- Concerto's targeted app tests still pass against the existing terminal workspace UI.

## Intent

Polish the tmux attach follow-up so the remaining diff is smaller and easier to trust. The Rust test fixture now matches real tmux metadata more closely, and the Swift side keeps the attach launch contract explicit without changing behavior.

## Assumptions

- Only tmux-backed sessions should have a tmux session name.
- Tmux attach still launches without extra environment variables.
- This branch is a helper/test polish pass, not a new daemon or client protocol change.

## Key decisions

- Gate tmux fixture data on `TMUX_TERMINAL_SOURCE` in the Rust route tests.
- Keep `TerminalAttachCommand` as a full `{workingDirectory, argv, env}` bundle for Ghostty launch sites.
- Preserve the existing terminal workspace dark background instead of mixing in a new visual change.

## Not included

- No new attach behavior.
- No HTTP contract changes.
- No broader executor or end-to-end regression sweep beyond the targeted checks above.
