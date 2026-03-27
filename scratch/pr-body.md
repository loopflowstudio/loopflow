## Try it!

```bash
swift test --package-path swift
cd swift && xcodegen generate && xcodebuild build -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO
```

Then launch Concerto, select a local wave workspace, and try the new shell flows:

- **Open Terminal** opens an external Ghostty window attached to `lf-<waveId>-shell`
- **Open Internally** selects the same tmux-backed shell in Concerto's embedded Ghostty surface
- Stream an assistant response and watch the transcript update without view-level regrouping or timestamp rescans

Validation from this gate pass:

- `swift test --package-path swift` ✅
- `xcodebuild build ... -scheme Concerto` ✅
- `xcodebuild test ... -skip-testing:ConcertoUITests` ✅
- `xcodebuild test ... -scheme Concerto` ⚠️ package tests + in-process Concerto suites passed, but `ConcertoUITests-Runner` crashed before `ScreenshotPipelineTests` established a UI-test connection in this environment

## Intent

This change makes the chat session hot path incremental instead of recomputing transcript-derived state on every streaming token, and it makes Ghostty the consistent external-shell path for local worktrees. The result should be smoother token streaming and a tighter loop between Concerto's workspace sidebar, embedded terminal, and external terminal window.

## Assumptions

- Assistant streaming remains append-only, so content-length invalidation is sufficient for cached message segment parsing.
- Local Ghostty is installed at `/Applications/Ghostty.app` and tmux is available for the shared shell session.
- The new workspace shell buttons are only expected to work for local worktrees; remote targets keep their existing SSH/IDE behavior.
- Current Ghostty CLI builds support `--window-inherit-working-directory=false` and `--initial-command=shell:...`.

## Key decisions

- Put `groupedTranscript`, `latestAssistantMessageId`, `timestampLabels`, and the transcript reverse index in `SessionState` so mutation sites maintain derived state once.
- Introduce `TerminalApp.defaultExternal` and route every default terminal entry point through it instead of leaving a mix of `.warp` and `.ghostty` call sites.
- Launch Ghostty with first-surface-specific arguments so reused Ghostty processes still honor the requested working directory and attach command.
- Keep the embedded/external shell pairing simple: one tmux session name per wave (`lf-<waveId>-shell`) and two attach surfaces.

## Not included

- No remote Ghostty/tmux workflow or detached-session reattach UI beyond the local workspace buttons.
- No Instruments capture in this headless pass.
- No markdown rendering or conversation-history work.
