# 01: Streaming Performance

Stop the bleeding. The chat UI re-computes everything on every SSE token. Fix the hot path.

## Done when

Instruments Time Profile shows no frame drops during a 100-message streaming session at 30 tokens/sec. `groupedTranscript`, `timestampLabelByMessageId`, and `parseMessageSegments` do not appear in the top 10 callers during streaming.

## What to fix

### Cache derived transcript state

`WaveSessionView` computes three O(n) properties on every `body` evaluation:

- `groupedTranscript` — scans full transcript to group tool calls
- `latestAssistantMessageId` — reverse scan for last assistant message
- `timestampLabelByMessageId` — scans full transcript for gap labels

Move these to `@State` cached values updated via `onChange(of: state.transcript.count)` for structural changes, and a targeted update for in-place delta mutations. The key insight: during streaming, only the *last* group and *last* message change. Don't recompute the whole list.

### Memoize message parsing

`parseMessageSegments` runs in `MessageRow.body` on every render — re-parsing the full content string on every token delta, for every visible row.

Options:
- Move parsed segments into `SessionState` (compute once on mutation, not on render)
- Cache in `MessageRow` keyed on content length (cheap staleness check)
- Use `EquatableView` wrapper so SwiftUI skips re-rendering unchanged rows

### Add Equatable to TranscriptGroup

`TranscriptGroup` is `Identifiable` but not `Equatable`. SwiftUI's `ForEach` can't skip unchanged rows without value equality. Add `Equatable` conformance so only the actively-streaming group re-renders.

### Fix scroll animation

Line 71 of `WaveSessionView.swift`:
```swift
withAnimation {
    proxy.scrollTo(last.id, anchor: .bottom)
}
```

- Uses bare `withAnimation` — doesn't respect `reduceMotion`
- Stacking scroll animations during rapid appends causes jitter
- Switch to `DesignAnimation.fast(reduceMotion)` or `nil` (instant scroll during streaming)

### Prevent StreamingCursorView flicker

If SwiftUI recreates the view identity during an in-place transcript mutation, the blinking animation restarts — visible as a flicker on each delta. Pin the cursor view identity with `.id("streaming-cursor")` or move the animation state up.

## Constraints

- Don't restructure `SessionState` beyond what's needed for caching. Larger refactors are stage 3+.
- Keep the `@Observable` pattern — don't regress to `ObservableObject` / `@Published`.
- All changes must pass existing Concerto UI tests.
