# swiftchatui: Streaming Performance + Open in Ghostty

Two focused deliverables on this branch. Both are stage 0/1 of their respective waves.

---

## Part 1: Chat UI Streaming Performance (chatgui stage 1)

Fix the per-token hot path in WaveSessionView so streaming feels instant. Currently every SSE delta triggers three O(n) scans, re-parses all visible message content, and can't skip unchanged rows.

### What's wrong

Every SSE token mutates `transcript[index]`, which triggers `WaveSessionView.body`, which runs:

1. `groupedTranscript` — O(n) scan to group tool calls
2. `latestAssistantMessageId` — O(n) reverse scan
3. `timestampLabelByMessageId` — O(n) scan for gap labels
4. `parseMessageSegments` in every visible `MessageRow` — re-parses full content per delta

At 20 tokens/sec with 50 entries, this is the source of jank.

### Data structures

```swift
// Cached transcript grouping — updated incrementally, not recomputed
@State private var cachedGroups: [TranscriptGroup] = []
@State private var cachedLatestAssistantId: UUID?
@State private var cachedTimestampLabels: [UUID: String] = [:]

// Parsed segments cache on SessionMessage or MessageRow
// Key: message id + content count (cheap staleness)
struct ParsedMessageCache {
    let messageId: UUID
    let contentLength: Int
    let segments: [MessageSegment]
}
```

### Key functions

```swift
// Incremental group update — only reprocess tail of transcript
func updateGroupsIncrementally(transcript: [TranscriptEntry], existing: inout [TranscriptGroup])

// Equatable on TranscriptGroup so ForEach can skip unchanged rows
extension TranscriptGroup: Equatable { ... }
```

### What to fix

- Cache `groupedTranscript`, `latestAssistantMessageId`, `timestampLabelByMessageId` as `@State`, update via `onChange` — during streaming only the last group/message changes
- Memoize `parseMessageSegments` — cache in MessageRow keyed on content length, or move parsing into SessionState
- Add `Equatable` to `TranscriptGroup` so `ForEach` can skip unchanged rows
- Fix bare `withAnimation` on scroll (line 71) — use `DesignAnimation.fast(reduceMotion)` or `nil` during streaming
- Pin `StreamingCursorView` identity to prevent animation restart flicker on each delta

### Constraints

- Stay on `@Observable` — no regression to `ObservableObject`
- Existing Concerto UI tests must pass
- Don't restructure SessionState beyond adding cached/derived properties

### Done when

- Instruments Time Profile during 100-message streaming at 30 tok/s shows no frame drops
- `groupedTranscript`, `timestampLabelByMessageId`, `parseMessageSegments` absent from top 10 callers during streaming
- Scroll animation respects `reduceMotion`
- StreamingCursorView doesn't flicker on each delta

---

## Part 2: Open in Ghostty (chattui stage 0)

A button on the wave workspace that opens a Ghostty terminal at the repo path, backed by a tmux session so you can find it again.

### What to build

- "Open Terminal" button on workspace (icon + keyboard shortcut)
- Creates a named tmux session (e.g. `lf-<wave-id>-shell`) via existing `TmuxSession`
- Opens external Ghostty window attached to that session:
  ```bash
  /Applications/Ghostty.app/Contents/MacOS/ghostty \
    --working-directory=/path \
    --command="tmux attach -t lf-<wave-id>-shell"
  ```
- "Open Internally" option attaches the same tmux session in Concerto's embedded Ghostty view
- If the tmux session already exists, attach to it instead of creating a new one
- Register session in `TmuxSessionRegistry` for cleanup and reattach
- Add `launchGhostty` case to `TerminalLauncher` alongside existing Warp/iTerm/Kitty

### What exists

- `TmuxSession.swift` — create/attach/kill tmux sessions
- `TmuxSessionRegistry.swift` — track active sessions
- `TerminalLauncher.swift` — Warp, iTerm, Terminal.app, Kitty (no Ghostty yet)
- `GhosttyTerminalView.swift` — embedded terminal via libghostty
- Ghostty at `/Applications/Ghostty.app/Contents/MacOS/ghostty`

### Done when

- Clicking "Open Terminal" on a wave opens a Ghostty window `cd`'d to the repo
- Closing the window doesn't kill the session
- Clicking "Open Internally" attaches the same session in Concerto's embedded view
- Session appears in Concerto's session list with correct status
