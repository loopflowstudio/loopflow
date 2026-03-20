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

## Intent

Make the embedded Ghostty pane behave like a terminal first. Ordinary typing should go through `ghostty_surface_key`, while IME commit and explicit paste stay on `ghostty_surface_text`, so TUIs stop seeing normal typing as injected text.

## Assumptions

- Input sources whose identifiers contain `inputmethod` should stay on the AppKit text-input path so IME composition can start before marked text exists.
- Option-modified printable keys should remain on the text-input path to preserve dead-key accent composition on Roman layouts.
- Inside terminal focus, Concerto should only keep the intentional multiplexer shortcuts; everything else should fall through to Ghostty.

## Key decisions

- Added a direct-key fast path in `GhosttyMetalView` for ordinary printable typing.
- Kept `insertText` narrow: IME commits and explicit paste still use text insertion semantics.
- Kept terminal-mode shortcut interception scoped to `.multiplexer` actions only.
- Polished `KeyboardRouter` so the help overlay still dismisses correctly even when a Ghostty pane owns first responder.

## Not included

- No new remote terminal transport or daemon-owned PTY work.
- No broader shortcut remapping beyond the terminal/multiplexer boundary.
- No fix yet for the existing macOS UI screenshot test runner bootstrap crash on this host (`ConcertoUITests-Runner` exits before XCTest connects).
