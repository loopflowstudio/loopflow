## Try it!

```bash
swift test --package-path swift
cd swift && xcodegen generate && xcodebuild build -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO
```

Then launch Concerto, select a local wave workspace, and try the shell/streaming flows:

- **Open Terminal** opens Ghostty attached to `lf-<waveId>-shell`
- **Open Internally** selects the same tmux-backed shell in Concerto's embedded Ghostty surface
- Stream an assistant response and watch the transcript update without view-level regrouping or timestamp rescans

Validation from this gate pass:

- `swift test --package-path swift` ✅
- `cd swift && xcodegen generate && xcodebuild build -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO` ✅
- `cd swift && xcodegen generate && xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO -skip-testing:ConcertoUITests` ✅
- `cd swift && xcodegen generate && xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO` ⚠️ package tests + in-process Concerto suites passed, but `ConcertoUITests-Runner` exited early before establishing the UI-test connection in this environment

## Intent

This change moves transcript hot-path derivation out of the view layer and into session state, while also making Ghostty the consistent external-shell path for local workspaces. The result should be smoother streaming, a stable sidebar/workspace shell story, and reviewer-visible docs that match the shipped shortcuts.

## Assumptions

- Assistant streaming remains append-only, so content-length invalidation is enough for cached message segment parsing.
- Local Ghostty is installed at `/Applications/Ghostty.app` and tmux is available.
- The new workspace shell actions only target local worktrees; remote targets keep the existing SSH/IDE behavior.
- Current Ghostty CLI builds support `--window-inherit-working-directory=false` and `--initial-command=shell:...`.

## Key decisions

- Keep `groupedTranscript`, `latestAssistantMessageId`, `timestampLabels`, and the transcript reverse index in `SessionState` so mutation sites update derived state once.
- Introduce `TerminalApp.defaultExternal` and route default terminal launches through it.
- Model the workspace shell as one tmux session per wave (`lf-<waveId>-shell`) shared by external and embedded terminal surfaces.
- Preserve existing sidebar ordering across server refreshes instead of letting refresh payload order reshuffle waves.

## Not included

- No remote Ghostty/tmux workflow or detached-session reattach UI.
- No Instruments capture in this headless pass.
- No markdown-rendering or conversation-history feature work beyond the streaming-path cache.
