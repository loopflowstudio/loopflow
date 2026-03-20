## Try it!

```bash
swift test --package-path swift --filter GhosttyTerminalViewTests
swift test --package-path swift --filter KeyboardRouterTests
cd swift && xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO -only-testing:ConcertoTests
```

Then launch Concerto, focus an embedded terminal pane, and verify:
- ordinary typing in a shell prompt or Claude Code feels character-by-character instead of paste-like
- `Ctrl-b`, `Ctrl-c`, `Ctrl-l`, `Esc`, and arrows still reach the terminal
- `Cmd-V` still pastes
- pane shortcuts like `⌘\\`, `⇧⌘↩`, and `⌥⌘←/→` still stay app-owned

