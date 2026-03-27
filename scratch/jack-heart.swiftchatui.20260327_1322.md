# chatgui stage 1: Streaming Performance

## What to build

Fix the per-token hot path in WaveSessionView so streaming feels instant. Currently every SSE delta triggers three O(n) scans, re-parses all visible message content, and can't skip unchanged rows.

## Data structures

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

## Key functions

```swift
// Incremental group update — only reprocess tail of transcript
func updateGroupsIncrementally(transcript: [TranscriptEntry], existing: inout [TranscriptGroup])

// Equatable on TranscriptGroup so ForEach can skip unchanged rows
extension TranscriptGroup: Equatable { ... }
```

## Constraints

- Must stay on `@Observable` — no regression to `ObservableObject`
- Existing Concerto UI tests must pass
- Don't restructure SessionState beyond adding cached/derived properties

## Done when

- Instruments Time Profile during 100-message streaming at 30 tok/s shows no frame drops
- `groupedTranscript`, `timestampLabelByMessageId`, `parseMessageSegments` absent from top 10 callers
- Scroll animation uses `DesignAnimation.fast(reduceMotion)`, not bare `withAnimation`
- StreamingCursorView doesn't flicker on each delta
