# Review: tmux attach helper polish

## What was implemented

- Updated the Rust terminal-session test fixture so only tmux-backed sessions get a synthetic `tmux_name`.
- Kept Concerto's tmux attach launch shape explicit by carrying `workingDirectory`, `argv`, and `env` together in `TerminalAttachCommand`.
- Added Swift assertions that both local and remote tmux attach commands keep an empty environment.
- Preserved the existing dark terminal background while polishing the helper code.

## Key choices

- Used `source == TMUX_TERMINAL_SOURCE` in the Rust fixture instead of always populating tmux metadata, so the negative attach test mirrors real session data more closely.
- Kept `env` on the Swift attach command struct even though it is currently empty, because Ghostty launch sites already consume a three-part command contract.
- Avoided broader cleanup in terminal attach code; this pass stays scoped to helper realism and reviewability.

## How it fits together

The Rust route tests exercise the daemon-side attach preconditions, while `TerminalAttachCommand` in Concerto turns `TerminalConnectionInfo` into the exact launch tuple Ghostty consumes. The branch only adjusts those helper layers, so reviewers can reason about attach behavior without re-reading the larger tmux transport change.

## Risks and bottlenecks

- If tmux attach ever needs environment variables, `TerminalAttachCommand.env` must become real data instead of a fixed empty dictionary.
- Validation is targeted; it does not re-run the full repo CI matrix.
- The slowest check is the targeted Xcode test run.

## What's not included

- No HTTP/API contract changes.
- No tmux lifecycle or SSH behavior changes.
- No README or end-user workflow changes.

## Validation

- `cargo fmt --check` ✅
- `cargo clippy -p loopflow -- -D warnings` ✅
- `cargo test -p loopflow terminal_sessions -- --nocapture` ✅
- `swift test --package-path swift --filter GhosttyTerminalViewTests` ✅
- `swift test --package-path swift --filter LocalWaveServiceAuthTests` ✅
- `cd swift && xcodegen generate` ✅
- `cd swift && xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO -only-testing:ConcertoTests/GhosttyTerminalViewTests -only-testing:ConcertoTests/RepoStateInteractiveSessionTests` ✅
