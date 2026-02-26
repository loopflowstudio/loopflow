# Review: Quote Replies (macOS)

## What was implemented

Quote-reply workflow for Concerto's wave session view on macOS. Users select text in assistant messages, get a popover with reply/emoji options, queue multiple replies, then send them as a single structured message.

**New files (5):**
- `ReplyQueue.swift` — data model: `ReplyEntry` enum + `ReplyQueue` observable with add/remove/clear/assemble
- `ReplyDraftTray.swift` — collapsible tray showing queued items; `ReplyComposerPopover` for text + emoji input
- `SelectableAssistantMessageTextView.swift` — macOS `NSViewRepresentable` wrapping a read-only, auto-sizing NSTextView that fires selection change callbacks
- `ReplyDemoView.swift` — prototype gallery for comparing interaction styles (popover vs inline, fixed vs picker emoji)
- `ReplyQueueTests.swift` — 8 tests covering assembly, sanitization, removal, CRLF normalization, edge cases

**Modified files (4):**
- `WaveSessionView.swift` — wired reply queue state, draft tray, and popover into the session transcript
- `ConcertoApp.swift` — added Reply Demo window + Debug menu entry (Shift+Cmd+R)
- `swift/README.md` — documented session quote replies section
- `wave/mobile/` — milestone doc and roadmap update

## Key choices

| Decision | Why | Alternative rejected |
|----------|-----|---------------------|
| Queue-then-send (not send-on-react) | Users reviewing LLM output queue several replies before committing. One assembled message is easier for the agent to parse. | Immediate send on each reaction — loses the "review before sending" benefit |
| `NSViewRepresentable` for selection | SwiftUI's `Text` doesn't expose selection ranges. AppKit NSTextView gives full control over selection callbacks. | SwiftUI `.textSelection(.enabled)` — no programmatic access to selected range |
| Fixed emoji palette (👍👎✏️❓) | Fast, discoverable, no extra UI. Customizable picker deferred to dogfooding feedback. | Full emoji picker — more flexible but heavier UI for a v1 |
| Plain text in selectable view | Markdown rendering in NSTextView is complex (needs AttributedString pipeline). Shipped plain text now, listed markdown fidelity as follow-up. | Render markdown — correct but high effort for this milestone |
| Separate `ReplyDemoView` window | Decouples prototype exploration from the live session. Lets designers screenshot options without wiring a real session. | Inline in session — mixes debug and production UI |

## How it fits together

```
Assistant message text (NSTextView)
  → user selects text → SelectableAssistantMessageTextView fires onSelectionChanged
    → MessageRow shows ReplyComposerPopover anchored to selection
      → user types reply or taps emoji → ReplyQueue.add(...)
        → ReplyDraftTray appears showing queued entries
          → user hits Send → ReplyQueue.assembleMessage() → SessionState.send()
```

The `ReplyQueue` is the single source of truth. Views read from it, mutate through its methods, and the composer bar assembles the final message. The queue is `@Observable`, so SwiftUI re-renders automatically.

## Risks and bottlenecks

- **Plain text selection** — `SelectableAssistantMessageTextView` renders assistant content without markdown formatting. Long messages with code blocks or lists look degraded compared to the default `AttributedString` rendering in non-selectable messages. This is the biggest UX gap.
- **macOS only** — iOS has no equivalent selection gesture wired up. The `#if os(macOS)` blocks in `WaveSessionView` are a documented exception to the multiplatform rules but need resolution.
- **No reorder/edit** — queued entries can only be added or deleted. If a user queues 5 replies and wants to reorder them, they must delete and re-add.
- **`groupedTranscript` recomputes on every render** — O(n) grouping runs on each SwiftUI body evaluation. Not a problem at current transcript sizes but could matter for long sessions.

## What's not included

- iOS quote selection (deferred — needs long-press gesture design)
- Markdown rendering in selectable text view (follow-up)
- Queued entry reorder/edit controls (follow-up)
- Emoji picker customization (deferred to dogfooding feedback)
- Capture scope beyond assistant messages (decision pending)

## Gate polish applied

- Fixed `highlightedQuote` capitalization mismatch in `ReplyDemoView`
- Added accessibility labels to emoji reaction buttons in `ReplyComposerPopover`
- Added `.minHitTarget()` to expand/collapse button in `ReplyDraftTray`
- Added `.accessibilityHidden(true)` to decorative accent bar in `MessageRow`
- Added 4 tests: `remove(id:)`, `clear()`, empty-queue-with-extra-text, CRLF normalization
