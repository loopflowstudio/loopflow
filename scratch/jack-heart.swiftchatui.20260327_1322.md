# swiftchatui: Streaming Performance + Open in Ghostty

## Try it

```bash
swift test --package-path swift
cd swift && xcodegen generate && xcodebuild build -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO
```

Launch Concerto, select a local wave workspace:

- **Open Terminal** opens an external Ghostty window attached to `lf-<waveId>-shell`
- **Open Internally** selects the same tmux-backed shell in Concerto's embedded Ghostty surface
- Stream an assistant response and watch the transcript update without view-level regrouping or timestamp rescans

## Validate

**Streaming performance:**
- Instruments Time Profiler during 100-message streaming at 30 tok/s: no frame drops
- `groupedTranscript`, `timestampLabelByMessageId`, `parseMessageSegments` absent from top 10 callers during streaming
- Scroll animation respects `reduceMotion`
- StreamingCursorView doesn't flicker on each delta

```bash
cd swift && xcodegen generate && xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO
```

**Ghostty shell:**
- Click "Open Terminal" on workspace → Ghostty window appears within 1s
- Run `tmux ls` → session `lf-<waveId>-shell` exists
- Close Ghostty → `tmux ls` still shows the session
- Click "Open Internally" → embedded terminal shows the same shell

## Measure

**Part 1 — Instruments before/after:**
- Baseline: record Time Profiler during a streaming session. Note frame drops, top callers.
- After: same recording. Target: 0 frame drops at 30 tok/s with 100 entries.

**Part 2 — functional:**
- Click "Open Terminal" → Ghostty window appears within 1s
- `tmux ls` shows `lf-<waveId>-shell`
- Close Ghostty → session persists
- "Open Internally" → embedded terminal shows same shell
