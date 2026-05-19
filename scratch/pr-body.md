## Try it!

```bash
cargo fmt --check
cargo clippy -- -D warnings
cargo test --all
uv run pytest python/tests/ -q
swift test --package-path swift
uv run python scripts/verify_embedded_build_driver.py --skip-build
```

Expected: palette terminal launch creates an lfd terminal session, the scripted missing flow exits `failed`, and the tmux session stays attachable.

I also ran:

```bash
cd swift && xcodegen generate && xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO
```

The Swift package/unit tests passed, then the UI runner failed to bootstrap in this headless environment (`ConcertoUITests-Runner ... Early unexpected exit`, xcresult: `/Users/jack/Library/Developer/Xcode/DerivedData/LoopflowSwift-ggnkujcptqdpggcjmdhzstcgxgez/Logs/Test/Test-Concerto-2026.05.18_19-45-34--0700.xcresult`).

## Intent

Make Concerto a first-class Desktop surface for daily build work while starting the native-chat polish pass. Flow launches now go through lfd-owned embedded terminal sessions that survive daemon/app boundaries, and assistant messages render as native markdown blocks with syntax-colored code and diff views.

## Assumptions

- tmux remains the process/session host for embedded build-driver terminals.
- Concerto terminal panes should persist lfd terminal session ids, not client-generated tmux names or launch commands.
- Chat rendering should stay dependency-free and split streaming/finalized work so active streams stay cheap.

## Key decisions

- Palette terminal sessions are lfd-owned and use the existing attach contract; Swift only binds `terminalSessionId` into pane config.
- Palette completion is exit-file based so the flow can finish while tmux stays open as an attachable shell.
- Markdown parsing/highlighting lives in `LoopflowCore`; Concerto owns visual rendering.
- Diff/patch fenced blocks reuse `DiffLinesView` instead of a separate assistant-message diff renderer.

## Not included

- Conversation history UI/API (native-chat M2).
- Composer file drop and slash commands (native-chat M3).
- Uploading files or sandbox-copying external files into an agent cwd.
- IDE-grade syntax highlighting or a third-party markdown renderer.
